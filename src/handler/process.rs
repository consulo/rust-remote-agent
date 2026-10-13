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

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tonic::{Status, Streaming};

use crate::error::{AgentResult, io_err};
use crate::proto::exec_event::Event;
use crate::proto::exec_input::Input;
use crate::proto::{ExecEvent, ExecExit, ExecInput, ExecStart, ProcessInfo};

const READ_CHUNK: usize = 8192;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub type EventSender = mpsc::Sender<Result<ExecEvent, Status>>;

struct ManagedProcess {
    command: String,
    kill: mpsc::UnboundedSender<bool>,
}

#[derive(Clone)]
pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<i64, ManagedProcess>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        ProcessManager {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts the process and drives it: output goes to `events`, `input` feeds stdin and kill requests.
    pub fn exec(&self, start: ExecStart, input: Streaming<ExecInput>, events: EventSender) -> AgentResult<i64> {
        let mut command = Command::new(&start.command);
        command
            .args(&start.arguments)
            .envs(&start.environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if !start.working_directory.is_empty() {
            command.current_dir(&start.working_directory);
        }
        // own process group, so a kill reaches the whole tree
        #[cfg(unix)]
        command.process_group(0);

        let mut child = command
            .spawn()
            .map_err(|e| io_err(format!("Failed to start process '{}'", start.command), e))?;
        let pid = child.id().map(i64::from).unwrap_or(-1);

        let (kill_tx, kill_rx) = mpsc::unbounded_channel();
        self.processes.lock().unwrap().insert(
            pid,
            ManagedProcess {
                command: start.command.clone(),
                kill: kill_tx.clone(),
            },
        );
        log::info!("Started process pid={} cmd={}", pid, start.command);

        let _ = events.try_send(Ok(ExecEvent {
            event: Some(Event::Started(pid)),
        }));

        let stdout = child.stdout.take().map(|out| tokio::spawn(forward(out, events.clone(), Event::Stdout)));
        let stderr = child.stderr.take().map(|err| tokio::spawn(forward(err, events.clone(), Event::Stderr)));
        tokio::spawn(feed_input(input, child.stdin.take(), kill_tx));

        let manager = self.clone();
        tokio::spawn(async move {
            let exit = wait(&mut child, pid, kill_rx).await;
            // a descendant can keep the pipes open after the process is gone
            for mut reader in [stdout, stderr].into_iter().flatten() {
                if tokio::time::timeout(DRAIN_TIMEOUT, &mut reader).await.is_err() {
                    reader.abort();
                }
            }
            manager.processes.lock().unwrap().remove(&pid);
            log::info!("Process pid={} exited {:?}", pid, exit);
            let _ = events
                .send(Ok(ExecEvent {
                    event: Some(Event::Exited(exit)),
                }))
                .await;
        });

        Ok(pid)
    }

    pub fn kill_process(&self, pid: i64, force: bool) -> bool {
        match self.processes.lock().unwrap().get(&pid) {
            Some(process) => process.kill.send(force).is_ok(),
            None => false,
        }
    }

    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        self.processes
            .lock()
            .unwrap()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: *pid,
                alive: true,
                command: process.command.clone(),
            })
            .collect()
    }
}

async fn forward<R: AsyncRead + Unpin>(mut source: R, events: EventSender, wrap: fn(Vec<u8>) -> Event) {
    let mut buffer = vec![0u8; READ_CHUNK];
    loop {
        match source.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let event = ExecEvent {
                    event: Some(wrap(buffer[..n].to_vec())),
                };
                if events.send(Ok(event)).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn feed_input(mut input: Streaming<ExecInput>, mut stdin: Option<ChildStdin>, kill: mpsc::UnboundedSender<bool>) {
    loop {
        match input.message().await {
            Ok(Some(message)) => match message.input {
                Some(Input::Stdin(data)) => {
                    if let Some(pipe) = stdin.as_mut() {
                        if pipe.write_all(&data).await.is_err() || pipe.flush().await.is_err() {
                            stdin = None;
                        }
                    }
                }
                Some(Input::CloseStdin(_)) => stdin = None,
                Some(Input::Kill(request)) => {
                    let _ = kill.send(request.force);
                }
                Some(Input::Start(_)) | None => {}
            },
            Ok(None) => {
                // half-close: no more input, the process keeps running
                return;
            }
            Err(_) => {
                // the client went away: nobody can see the output any more
                let _ = kill.send(true);
                return;
            }
        }
    }
}

async fn wait(child: &mut Child, pid: i64, mut kill: mpsc::UnboundedReceiver<bool>) -> ExecExit {
    loop {
        tokio::select! {
            status = child.wait() => {
                return match status {
                    Ok(status) => exit_of(status),
                    Err(e) => {
                        log::warn!("Failed to wait for pid={}: {}", pid, e);
                        ExecExit { exit_code: None, signal: None }
                    }
                };
            }
            Some(force) = kill.recv() => {
                let result = kill_tree(child, pid, force).await;
                if let Err(e) = result {
                    log::warn!("Failed to kill pid={} force={}: {}", pid, force, e);
                }
            }
        }
    }
}

#[cfg(unix)]
fn exit_of(status: std::process::ExitStatus) -> ExecExit {
    use std::os::unix::process::ExitStatusExt;
    ExecExit {
        exit_code: status.code(),
        signal: status.signal(),
    }
}

#[cfg(not(unix))]
fn exit_of(status: std::process::ExitStatus) -> ExecExit {
    ExecExit {
        exit_code: status.code(),
        signal: None,
    }
}

/// Unix: SIGTERM or SIGKILL to the process group. Windows: taskkill /T, with /F when forced.
#[cfg(unix)]
async fn kill_tree(_child: &mut Child, pid: i64, force: bool) -> std::io::Result<()> {
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    // SAFETY: plain syscall on the process group this agent created
    let result = unsafe { libc::kill(-(pid as i32), signal) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
async fn kill_tree(child: &mut Child, pid: i64, force: bool) -> std::io::Result<()> {
    let mut command = Command::new("taskkill");
    command.args(["/T", "/PID", &pid.to_string()]);
    if force {
        command.arg("/F");
    }
    let status = command.stdout(Stdio::null()).stderr(Stdio::null()).status().await?;
    if status.success() {
        return Ok(());
    }
    if force {
        return child.start_kill();
    }
    Err(std::io::Error::other(format!("taskkill failed for pid {}", pid)))
}
