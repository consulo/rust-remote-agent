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
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::generated::remote_agent::{AgentException, ProcessInfo, ProcessOutput};

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

/// Ring-style buffer that accumulates output from a background reader thread.
/// The client reads new data since last poll via `drain()`.
struct OutputBuffer {
    data: Vec<u8>,
    cursor: usize,
}

impl OutputBuffer {
    fn new() -> Self {
        OutputBuffer {
            data: Vec::new(),
            cursor: 0,
        }
    }

    fn append(&mut self, chunk: &[u8]) {
        self.data.extend_from_slice(chunk);
    }

    /// Returns new data since last drain, advances cursor.
    fn drain(&mut self) -> Vec<u8> {
        if self.cursor >= self.data.len() {
            return Vec::new();
        }
        let new_data = self.data[self.cursor..].to_vec();
        self.cursor = self.data.len();
        new_data
    }
}

struct ManagedProcess {
    child: Child,
    command: String,
    stdout_buf: Arc<Mutex<OutputBuffer>>,
    stderr_buf: Arc<Mutex<OutputBuffer>>,
}

pub struct ProcessManager {
    processes: Mutex<BTreeMap<i64, ManagedProcess>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        ProcessManager {
            processes: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn start_process(
        &self,
        command: String,
        args: Vec<String>,
        working_directory: String,
        environment: BTreeMap<String, String>,
    ) -> thrift::Result<ProcessInfo> {
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if !working_directory.is_empty() {
            cmd.current_dir(&working_directory);
        }

        if !environment.is_empty() {
            cmd.envs(&environment);
        }

        let mut child = cmd.spawn().map_err(|e| {
            agent_err(format!("Failed to start process '{}': {}", command, e))
        })?;

        let pid = child.id() as i64;

        // Take ownership of stdout/stderr and spawn reader threads
        let stdout_buf = Arc::new(Mutex::new(OutputBuffer::new()));
        let stderr_buf = Arc::new(Mutex::new(OutputBuffer::new()));

        if let Some(stdout) = child.stdout.take() {
            let buf = Arc::clone(&stdout_buf);
            thread::spawn(move || {
                reader_thread(stdout, buf);
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let buf = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                reader_thread(stderr, buf);
            });
        }

        let info = ProcessInfo::new(pid, true, Some(command.clone()));

        self.processes.lock().unwrap().insert(
            pid,
            ManagedProcess {
                child,
                command,
                stdout_buf,
                stderr_buf,
            },
        );

        log::info!("Started process pid={} cmd={}", pid, info.command.as_deref().unwrap_or(""));
        Ok(info)
    }

    pub fn kill_process(&self, pid: i64, force: bool) -> thrift::Result<bool> {
        let mut processes = self.processes.lock().unwrap();
        let managed = match processes.get_mut(&pid) {
            Some(m) => m,
            None => return Ok(false),
        };

        let result = if force {
            // Hard kill: SIGKILL / TerminateProcess
            managed.child.kill()
        } else {
            // Soft kill: platform-specific graceful termination
            soft_kill(&managed.child)
        };

        match result {
            Ok(()) => {
                log::info!("Killed process pid={} force={}", pid, force);
                Ok(true)
            }
            Err(e) => {
                log::warn!("Failed to kill process pid={}: {}", pid, e);
                Ok(false)
            }
        }
    }

    pub fn is_alive(&self, pid: i64) -> bool {
        let mut processes = self.processes.lock().unwrap();
        if let Some(managed) = processes.get_mut(&pid) {
            matches!(managed.child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        let mut processes = self.processes.lock().unwrap();
        processes
            .iter_mut()
            .map(|(&pid, managed)| {
                let alive = matches!(managed.child.try_wait(), Ok(None));
                ProcessInfo::new(pid, alive, Some(managed.command.clone()))
            })
            .collect()
    }

    /// Returns new stdout/stderr data since last call (streaming poll).
    /// ANSI escape codes are preserved as raw bytes.
    pub fn read_output(&self, pid: i64) -> thrift::Result<ProcessOutput> {
        let mut processes = self.processes.lock().unwrap();
        let managed = processes.get_mut(&pid).ok_or_else(|| {
            agent_err(format!("Process not found: {}", pid))
        })?;

        let exit_code = match managed.child.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        };

        let stdout_data = managed.stdout_buf.lock().unwrap().drain();
        let stderr_data = managed.stderr_buf.lock().unwrap().drain();

        let mut output = ProcessOutput::default();
        if !stdout_data.is_empty() {
            output.stdout_data = Some(stdout_data);
        }
        if !stderr_data.is_empty() {
            output.stderr_data = Some(stderr_data);
        }
        output.exit_code = exit_code;

        Ok(output)
    }
}

/// Background thread that reads from a pipe into a shared buffer.
fn reader_thread<R: Read>(mut source: R, buf: Arc<Mutex<OutputBuffer>>) {
    let mut chunk = [0u8; 8192];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                buf.lock().unwrap().append(&chunk[..n]);
            }
            Err(e) => {
                log::debug!("Reader thread error: {}", e);
                break;
            }
        }
    }
}

/// Graceful termination: SIGTERM on Unix, Ctrl+C event on Windows.
#[cfg(unix)]
fn soft_kill(child: &Child) -> std::io::Result<()> {
    // Send SIGTERM
    unsafe {
        let ret = libc::kill(child.id() as i32, libc::SIGTERM);
        if ret != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn soft_kill(child: &Child) -> std::io::Result<()> {
    // Send Ctrl+C via GenerateConsoleCtrlEvent, fall back to taskkill /PID
    // GenerateConsoleCtrlEvent affects the whole process group, so use taskkill
    // which can target a specific PID gracefully.
    let pid = child.id();
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("taskkill failed for pid {}", pid),
        ))
    }
}
