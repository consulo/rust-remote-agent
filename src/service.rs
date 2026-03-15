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

use std::collections::{BTreeMap, HashSet};

use crate::generated::remote_agent::*;
use crate::handler;

const ALL_GROUPS: &[&str] = &["fs", "process", "http", "websocket", "userinfo"];

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

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

    fn check(&self, group: &str) -> thrift::Result<()> {
        if self.all || self.groups.contains(group) {
            Ok(())
        } else {
            Err(agent_err(format!(
                "Permission denied: '{}' not in --permissions",
                group
            )))
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

pub struct AgentServiceHandler {
    workspace: String,
    permissions: Permissions,
    process_manager: handler::process::ProcessManager,
    transfer_manager: handler::transfer::TransferManager,
    ws_manager: handler::websocket::WebSocketManager,
}

impl AgentServiceHandler {
    pub fn new(workspace: String, permissions: Permissions) -> Self {
        AgentServiceHandler {
            workspace,
            permissions,
            process_manager: handler::process::ProcessManager::new(),
            transfer_manager: handler::transfer::TransferManager::new(),
            ws_manager: handler::websocket::WebSocketManager::new(),
        }
    }
}

impl RemoteAgentServiceSyncHandler for AgentServiceHandler {
    // --- Agent Identity (no permission check) ---

    fn handle_get_agent_info(&self) -> thrift::Result<AgentInfo> {
        Ok(AgentInfo::new(
            "rust-remote-agent".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            self.permissions.to_list(),
        ))
    }

    // --- Workspace (no permission check) ---

    fn handle_get_workspace_path(&self) -> thrift::Result<String> {
        Ok(self.workspace.clone())
    }

    // --- Process Management (permission: process) ---

    fn handle_start_process(
        &self,
        command: String,
        arguments: Vec<String>,
        working_directory: String,
        environment: BTreeMap<String, String>,
    ) -> thrift::Result<ProcessInfo> {
        self.permissions.check("process")?;
        self.process_manager
            .start_process(command, arguments, working_directory, environment)
    }

    fn handle_kill_process(&self, pid: i64, force: bool) -> thrift::Result<bool> {
        self.permissions.check("process")?;
        self.process_manager.kill_process(pid, force)
    }

    fn handle_is_process_alive(&self, pid: i64) -> thrift::Result<bool> {
        self.permissions.check("process")?;
        Ok(self.process_manager.is_alive(pid))
    }

    fn handle_list_processes(&self) -> thrift::Result<Vec<ProcessInfo>> {
        self.permissions.check("process")?;
        Ok(self.process_manager.list_processes())
    }

    fn handle_read_process_output(&self, pid: i64) -> thrift::Result<ProcessOutput> {
        self.permissions.check("process")?;
        self.process_manager.read_output(pid)
    }

    // --- File Operations (permission: fs) ---

    fn handle_read_file(&self, path: String) -> thrift::Result<Vec<u8>> {
        self.permissions.check("fs")?;
        handler::file::read_file(&path)
    }

    fn handle_write_file(&self, path: String, data: Vec<u8>) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        handler::file::write_file(&path, &data)
    }

    fn handle_delete_file(&self, path: String) -> thrift::Result<bool> {
        self.permissions.check("fs")?;
        handler::file::delete_file(&path)
    }

    fn handle_list_directory(&self, path: String) -> thrift::Result<Vec<FileInfo>> {
        self.permissions.check("fs")?;
        handler::file::list_directory(&path)
    }

    fn handle_file_exists(&self, path: String) -> thrift::Result<bool> {
        self.permissions.check("fs")?;
        Ok(handler::file::file_exists(&path))
    }

    fn handle_create_directory(&self, path: String, recursive: bool) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        handler::file::create_directory(&path, recursive)
    }

    fn handle_list_roots(&self) -> thrift::Result<Vec<FileInfo>> {
        self.permissions.check("fs")?;
        Ok(handler::file::list_roots())
    }

    fn handle_set_permissions(&self, path: String, mode: i32) -> thrift::Result<bool> {
        self.permissions.check("fs")?;
        handler::file::set_permissions(&path, mode)
    }

    // --- File Transfer (permission: fs) ---

    fn handle_begin_upload(&self, path: String, file_size: i64) -> thrift::Result<String> {
        self.permissions.check("fs")?;
        self.transfer_manager.begin_upload(path, file_size)
    }

    fn handle_upload_chunk(&self, transfer_id: String, data: Vec<u8>) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        self.transfer_manager.upload_chunk(&transfer_id, &data)
    }

    fn handle_finish_upload(&self, transfer_id: String) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        self.transfer_manager.finish_upload(&transfer_id)
    }

    fn handle_cancel_upload(&self, transfer_id: String) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        self.transfer_manager.cancel_upload(&transfer_id)
    }

    fn handle_begin_download(&self, path: String) -> thrift::Result<DownloadInfo> {
        self.permissions.check("fs")?;
        self.transfer_manager.begin_download(&path)
    }

    fn handle_download_chunk(&self, transfer_id: String, chunk_size: i32) -> thrift::Result<Vec<u8>> {
        self.permissions.check("fs")?;
        self.transfer_manager.download_chunk(&transfer_id, chunk_size)
    }

    fn handle_finish_download(&self, transfer_id: String) -> thrift::Result<()> {
        self.permissions.check("fs")?;
        self.transfer_manager.finish_download(&transfer_id)
    }

    // --- Environment / System Info (no permission check) ---

    fn handle_get_env_variable(&self, name: String) -> thrift::Result<String> {
        Ok(handler::env::get_env_variable(&name))
    }

    fn handle_get_env_variables(&self) -> thrift::Result<BTreeMap<String, String>> {
        Ok(handler::env::get_env_variables())
    }

    fn handle_get_system_info(&self) -> thrift::Result<SystemInfo> {
        Ok(handler::env::get_system_info())
    }

    // --- User Info (permission: userinfo) ---

    fn handle_get_user_info(&self) -> thrift::Result<UserInfo> {
        self.permissions.check("userinfo")?;
        Ok(handler::env::get_user_info())
    }

    // --- HTTP Client (permission: http) ---

    fn handle_execute_http_request(
        &self,
        request: HttpRequest,
    ) -> thrift::Result<HttpResponse> {
        self.permissions.check("http")?;
        handler::http::execute_http_request(request)
    }

    // --- WebSocket Proxy (permission: websocket) ---

    fn handle_connect_web_socket(
        &self,
        url: String,
        headers: BTreeMap<String, String>,
    ) -> thrift::Result<String> {
        self.permissions.check("websocket")?;
        self.ws_manager.connect(url, headers)
    }

    fn handle_read_web_socket_data(
        &self,
        session_id: String,
    ) -> thrift::Result<WebSocketData> {
        self.permissions.check("websocket")?;
        self.ws_manager.read_data(&session_id)
    }

    fn handle_send_web_socket_data(
        &self,
        session_id: String,
        binary_data: Vec<u8>,
        text_data: String,
    ) -> thrift::Result<()> {
        self.permissions.check("websocket")?;
        if !binary_data.is_empty() {
            self.ws_manager.send_binary(&session_id, binary_data)
        } else {
            self.ws_manager.send_text(&session_id, text_data)
        }
    }

    fn handle_close_web_socket(
        &self,
        session_id: String,
    ) -> thrift::Result<()> {
        self.permissions.check("websocket")?;
        self.ws_manager.close(&session_id)
    }

    // --- Utility (no permission check) ---

    fn handle_find_free_port(&self) -> thrift::Result<i32> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| agent_err(format!("Failed to find free port: {}", e)))?;
        Ok(listener.local_addr().unwrap().port() as i32)
    }
}
