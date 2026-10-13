// Copyright 2013-2026 consulo.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashSet;
use std::pin::Pin;

use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use crate::error::{AgentError, AgentResult, agent_err};
use crate::handler;
use crate::proto::exec_input::Input as ExecInputKind;
use crate::proto::remote_agent_service_server::RemoteAgentService;
use crate::proto::web_socket_input::Input as WebSocketInputKind;
use crate::proto::*;

const ALL_GROUPS: &[&str] = &["fs", "process", "http", "websocket", "userinfo", "stat"];
const STREAM_BUFFER: usize = 64;

type EventStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

pub struct Permissions {
    all: bool,
    groups: HashSet<String>,
}

impl Permissions {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed == "*" {
            Permissions {
                all: true,
                groups: HashSet::new(),
            }
        } else {
            let groups = trimmed
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            Permissions {
                all: false,
                groups,
            }
        }
    }

    pub fn check(&self, group: &str) -> AgentResult<()> {
        if self.all || self.groups.contains(group) {
            Ok(())
        } else {
            Err(AgentError::Permission {
                message: format!("Permission denied: '{}' not in --permissions", group),
                permission: group.to_string(),
            })
        }
    }

    pub fn display(&self) -> String {
        if self.all {
            ALL_GROUPS.join(", ")
        } else {
            let mut sorted: Vec<&str> = self
                .groups
                .iter()
                .map(|s| s.as_str())
                .collect();
            sorted.sort();
            sorted.join(", ")
        }
    }

    pub fn to_list(&self) -> Vec<String> {
        if self.all {
            ALL_GROUPS.iter().map(|s| s.to_string()).collect()
        } else {
            let mut sorted: Vec<String> = self.groups.iter().cloned().collect();
            sorted.sort();
            sorted
        }
    }
}
pub struct AgentService {
    workspace: String,
    permissions: Permissions,
    process_manager: handler::process::ProcessManager,
}

impl AgentService {
    pub fn new(workspace: String, permissions: Permissions) -> Self {
        AgentService {
            workspace,
            permissions,
            process_manager: handler::process::ProcessManager::new(),
        }
    }
}

async fn blocking<T, F>(task: F) -> Result<Response<T>, Status>
where
    T: Send + 'static,
    F: FnOnce() -> AgentResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|e| Status::internal(format!("Agent task failed: {}", e)))?
        .map(Response::new)
        .map_err(Status::from)
}

fn stream<T: Send + 'static>(receiver: mpsc::Receiver<Result<T, Status>>) -> Response<EventStream<T>> {
    Response::new(Box::pin(ReceiverStream::new(receiver)))
}

#[tonic::async_trait]
impl RemoteAgentService for AgentService {
    async fn get_agent_info(&self, _request: Request<Empty>) -> Result<Response<AgentInfo>, Status> {
        Ok(Response::new(AgentInfo {
            agent_id: "rust-remote-agent".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            permissions: self.permissions.to_list(),
        }))
    }

    async fn get_workspace_path(&self, _request: Request<Empty>) -> Result<Response<WorkspacePath>, Status> {
        Ok(Response::new(WorkspacePath {
            path: self.workspace.clone(),
        }))
    }

    async fn get_env_variable(&self, request: Request<EnvVariableRequest>) -> Result<Response<EnvVariable>, Status> {
        Ok(Response::new(EnvVariable {
            value: handler::env::get_env_variable(&request.into_inner().name),
        }))
    }

    async fn get_env_variables(&self, _request: Request<Empty>) -> Result<Response<EnvVariables>, Status> {
        Ok(Response::new(EnvVariables {
            variables: handler::env::get_env_variables().into_iter().collect(),
        }))
    }

    async fn get_system_info(&self, _request: Request<Empty>) -> Result<Response<SystemInfo>, Status> {
        blocking(|| Ok(handler::env::get_system_info())).await
    }

    async fn get_stat(&self, _request: Request<Empty>) -> Result<Response<StatInfo>, Status> {
        self.permissions.check("stat")?;
        blocking(|| Ok(handler::env::get_stat())).await
    }

    async fn get_user_info(&self, _request: Request<Empty>) -> Result<Response<UserInfo>, Status> {
        self.permissions.check("userinfo")?;
        Ok(Response::new(handler::env::get_user_info()))
    }

    async fn find_free_port(&self, _request: Request<Empty>) -> Result<Response<FreePort>, Status> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| Status::from(agent_err(format!("Failed to find free port: {}", e))))?;
        let port = listener
            .local_addr()
            .map_err(|e| Status::from(agent_err(format!("Failed to find free port: {}", e))))?
            .port();
        Ok(Response::new(FreePort { port: port as i32 }))
    }

    type ExecStream = EventStream<ExecEvent>;

    async fn exec(&self, request: Request<Streaming<ExecInput>>) -> Result<Response<Self::ExecStream>, Status> {
        self.permissions.check("process")?;
        let mut input = request.into_inner();
        let start = match input.message().await? {
            Some(ExecInput {
                input: Some(ExecInputKind::Start(start)),
            }) => start,
            _ => {
                return Err(AgentError::InvalidArgument("The first message of exec must be start".to_string()).into());
            }
        };

        let (sender, receiver) = mpsc::channel(STREAM_BUFFER);
        self.process_manager.exec(start, input, sender)?;
        Ok(stream(receiver))
    }

    async fn kill_process(&self, request: Request<KillRequest>) -> Result<Response<KillResult>, Status> {
        self.permissions.check("process")?;
        let request = request.into_inner();
        Ok(Response::new(KillResult {
            killed: self.process_manager.kill_process(request.pid, request.force),
        }))
    }

    async fn list_processes(&self, _request: Request<Empty>) -> Result<Response<ProcessList>, Status> {
        self.permissions.check("process")?;
        Ok(Response::new(ProcessList {
            processes: self.process_manager.list_processes(),
        }))
    }

    async fn read_file(&self, request: Request<PathRequest>) -> Result<Response<FileContent>, Status> {
        self.permissions.check("fs")?;
        let path = request.into_inner().path;
        blocking(move || handler::file::read_file(&path).map(|data| FileContent { data })).await
    }

    async fn write_file(&self, request: Request<WriteFileRequest>) -> Result<Response<Empty>, Status> {
        self.permissions.check("fs")?;
        let request = request.into_inner();
        blocking(move || handler::file::write_file(&request.path, &request.data).map(|()| Empty {})).await
    }

    type ReadFileStreamStream = EventStream<FileContent>;

    async fn read_file_stream(
        &self,
        request: Request<ReadStreamRequest>,
    ) -> Result<Response<Self::ReadFileStreamStream>, Status> {
        self.permissions.check("fs")?;
        let request = request.into_inner();
        let file = handler::transfer::open_for_read(&request.path).await?;
        let (sender, receiver) = mpsc::channel(4);
        tokio::spawn(handler::transfer::read_stream(file, request.path, request.chunk_size, sender));
        Ok(stream(receiver))
    }

    async fn write_file_stream(&self, request: Request<Streaming<WriteStreamInput>>) -> Result<Response<Empty>, Status> {
        self.permissions.check("fs")?;
        handler::transfer::write_stream(request.into_inner()).await?;
        Ok(Response::new(Empty {}))
    }

    async fn delete_file(&self, request: Request<PathRequest>) -> Result<Response<DeleteResult>, Status> {
        self.permissions.check("fs")?;
        let path = request.into_inner().path;
        blocking(move || handler::file::delete_file(&path).map(|deleted| DeleteResult { deleted })).await
    }

    async fn list_directory(&self, request: Request<PathRequest>) -> Result<Response<FileInfoList>, Status> {
        self.permissions.check("fs")?;
        let path = request.into_inner().path;
        blocking(move || handler::file::list_directory(&path).map(|files| FileInfoList { files })).await
    }

    async fn file_exists(&self, request: Request<PathRequest>) -> Result<Response<ExistsResult>, Status> {
        self.permissions.check("fs")?;
        let path = request.into_inner().path;
        blocking(move || {
            Ok(ExistsResult {
                exists: handler::file::file_exists(&path),
            })
        })
        .await
    }

    async fn create_directory(&self, request: Request<CreateDirectoryRequest>) -> Result<Response<Empty>, Status> {
        self.permissions.check("fs")?;
        let request = request.into_inner();
        blocking(move || handler::file::create_directory(&request.path, request.recursive).map(|()| Empty {})).await
    }

    async fn list_roots(&self, _request: Request<Empty>) -> Result<Response<FileInfoList>, Status> {
        self.permissions.check("fs")?;
        blocking(|| {
            Ok(FileInfoList {
                files: handler::file::list_roots(),
            })
        })
        .await
    }

    async fn set_permissions(
        &self,
        request: Request<SetPermissionsRequest>,
    ) -> Result<Response<SetPermissionsResult>, Status> {
        self.permissions.check("fs")?;
        let request = request.into_inner();
        blocking(move || {
            handler::file::set_permissions(&request.path, request.mode).map(|changed| SetPermissionsResult { changed })
        })
        .await
    }

    async fn execute_http_request(&self, request: Request<HttpRequest>) -> Result<Response<HttpResponse>, Status> {
        self.permissions.check("http")?;
        let request = request.into_inner();
        blocking(move || handler::http::execute_http_request(request)).await
    }

    type WebSocketStream = EventStream<WebSocketEvent>;

    async fn web_socket(
        &self,
        request: Request<Streaming<WebSocketInput>>,
    ) -> Result<Response<Self::WebSocketStream>, Status> {
        self.permissions.check("websocket")?;
        let mut input = request.into_inner();
        let connect = match input.message().await? {
            Some(WebSocketInput {
                input: Some(WebSocketInputKind::Connect(connect)),
            }) => connect,
            _ => {
                return Err(AgentError::InvalidArgument("The first message of web_socket must be connect".to_string()).into());
            }
        };

        let socket = tokio::task::spawn_blocking(move || handler::websocket::connect(connect))
            .await
            .map_err(|e| Status::internal(format!("Agent task failed: {}", e)))??;
        let (sender, receiver) = mpsc::channel(STREAM_BUFFER);
        handler::websocket::run(socket, input, sender);
        Ok(stream(receiver))
    }
}
