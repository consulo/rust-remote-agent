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

use std::collections::BTreeMap;

use crate::generated::remote_agent::*;
use crate::handler;

pub struct AgentServiceHandler {
    workspace: String,
    process_manager: handler::process::ProcessManager,
    transfer_manager: handler::transfer::TransferManager,
}

impl AgentServiceHandler {
    pub fn new(workspace: String) -> Self {
        AgentServiceHandler {
            workspace,
            process_manager: handler::process::ProcessManager::new(),
            transfer_manager: handler::transfer::TransferManager::new(),
        }
    }
}

impl RemoteAgentServiceSyncHandler for AgentServiceHandler {
    // --- Agent Identity ---

    fn handle_get_agent_info(&self) -> thrift::Result<AgentInfo> {
        Ok(AgentInfo::new(
            "rust-remote-agent".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ))
    }

    // --- Workspace ---

    fn handle_get_workspace_path(&self) -> thrift::Result<String> {
        Ok(self.workspace.clone())
    }

    // --- Process Management ---

    fn handle_start_process(
        &self,
        command: String,
        arguments: Vec<String>,
        working_directory: String,
        environment: BTreeMap<String, String>,
    ) -> thrift::Result<ProcessInfo> {
        self.process_manager
            .start_process(command, arguments, working_directory, environment)
    }

    fn handle_kill_process(&self, pid: i64, force: bool) -> thrift::Result<bool> {
        self.process_manager.kill_process(pid, force)
    }

    fn handle_is_process_alive(&self, pid: i64) -> thrift::Result<bool> {
        Ok(self.process_manager.is_alive(pid))
    }

    fn handle_list_processes(&self) -> thrift::Result<Vec<ProcessInfo>> {
        Ok(self.process_manager.list_processes())
    }

    fn handle_read_process_output(&self, pid: i64) -> thrift::Result<ProcessOutput> {
        self.process_manager.read_output(pid)
    }

    // --- File Operations ---

    fn handle_read_file(&self, path: String) -> thrift::Result<Vec<u8>> {
        handler::file::read_file(&path)
    }

    fn handle_write_file(&self, path: String, data: Vec<u8>) -> thrift::Result<()> {
        handler::file::write_file(&path, &data)
    }

    fn handle_delete_file(&self, path: String) -> thrift::Result<bool> {
        handler::file::delete_file(&path)
    }

    fn handle_list_directory(&self, path: String) -> thrift::Result<Vec<FileInfo>> {
        handler::file::list_directory(&path)
    }

    fn handle_file_exists(&self, path: String) -> thrift::Result<bool> {
        Ok(handler::file::file_exists(&path))
    }

    fn handle_create_directory(&self, path: String, recursive: bool) -> thrift::Result<()> {
        handler::file::create_directory(&path, recursive)
    }

    fn handle_list_roots(&self) -> thrift::Result<Vec<FileInfo>> {
        Ok(handler::file::list_roots())
    }

    fn handle_set_permissions(&self, path: String, mode: i32) -> thrift::Result<bool> {
        handler::file::set_permissions(&path, mode)
    }

    // --- File Transfer (chunked) ---

    fn handle_begin_upload(&self, path: String, file_size: i64) -> thrift::Result<String> {
        self.transfer_manager.begin_upload(path, file_size)
    }

    fn handle_upload_chunk(&self, transfer_id: String, data: Vec<u8>) -> thrift::Result<()> {
        self.transfer_manager.upload_chunk(&transfer_id, &data)
    }

    fn handle_finish_upload(&self, transfer_id: String) -> thrift::Result<()> {
        self.transfer_manager.finish_upload(&transfer_id)
    }

    fn handle_cancel_upload(&self, transfer_id: String) -> thrift::Result<()> {
        self.transfer_manager.cancel_upload(&transfer_id)
    }

    fn handle_begin_download(&self, path: String) -> thrift::Result<DownloadInfo> {
        self.transfer_manager.begin_download(&path)
    }

    fn handle_download_chunk(&self, transfer_id: String, chunk_size: i32) -> thrift::Result<Vec<u8>> {
        self.transfer_manager.download_chunk(&transfer_id, chunk_size)
    }

    fn handle_finish_download(&self, transfer_id: String) -> thrift::Result<()> {
        self.transfer_manager.finish_download(&transfer_id)
    }

    // --- Environment / System Info ---

    fn handle_get_env_variable(&self, name: String) -> thrift::Result<String> {
        Ok(handler::env::get_env_variable(&name))
    }

    fn handle_get_env_variables(&self) -> thrift::Result<BTreeMap<String, String>> {
        Ok(handler::env::get_env_variables())
    }

    fn handle_get_system_info(&self) -> thrift::Result<SystemInfo> {
        Ok(handler::env::get_system_info())
    }

    fn handle_get_user_info(&self) -> thrift::Result<UserInfo> {
        Ok(handler::env::get_user_info())
    }
}
