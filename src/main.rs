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

mod generated;
mod handler;
mod platform;
mod service;

use clap::Parser;
use thrift::protocol::{TBinaryInputProtocolFactory, TBinaryOutputProtocolFactory};
use thrift::server::TServer;
use thrift::transport::{TBufferedReadTransportFactory, TBufferedWriteTransportFactory};

use crate::generated::remote_agent::RemoteAgentServiceSyncProcessor;
use crate::service::AgentServiceHandler;

const DEFAULT_PORT: u16 = 57638;

fn default_workspace() -> String {
    #[cfg(windows)]
    {
        // %LOCALAPPDATA%\consulo-workspace or C:\consulo-workspace
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            format!("{}\\consulo-workspace", local)
        } else {
            "C:\\consulo-workspace".to_string()
        }
    }
    #[cfg(not(windows))]
    {
        // ~/consulo-workspace — user-writable, no root needed
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/consulo-workspace", home)
        } else {
            "/tmp/consulo-workspace".to_string()
        }
    }
}

#[derive(Parser)]
#[command(name = "rust-remote-agent", version, about = "Remote agent for Consulo IDE")]
struct Cli {
    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Workspace root directory
    #[arg(long, default_value_t = default_workspace())]
    workspace: String,

    /// Run as a background daemon
    #[arg(long)]
    daemon: bool,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if cli.daemon {
        // TODO: platform-specific daemonization
        log::warn!("Daemon mode not yet implemented, running in foreground");
    }

    let listen_addr = format!("{}:{}", cli.host, cli.port);
    let workspace = cli.workspace;

    // Ensure workspace directory exists
    if let Err(e) = std::fs::create_dir_all(&workspace) {
        eprintln!("Failed to create workspace '{}': {}", workspace, e);
        std::process::exit(1);
    }

    log::info!("Starting remote-agent on {} (platform: {}, workspace: {})", listen_addr, platform::platform_label(), workspace);
    println!("rust-remote-agent v{}", env!("CARGO_PKG_VERSION"));
    println!("listening on {} (platform: {})", listen_addr, platform::platform_label());
    println!("workspace: {}", workspace);

    // Flush stdout before server.listen() blocks
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let handler = AgentServiceHandler::new(workspace);
    let processor = RemoteAgentServiceSyncProcessor::new(handler);

    let input_transport_factory = TBufferedReadTransportFactory::new();
    let input_protocol_factory = TBinaryInputProtocolFactory::new();
    let output_transport_factory = TBufferedWriteTransportFactory::new();
    let output_protocol_factory = TBinaryOutputProtocolFactory::new();

    let mut server = TServer::new(
        input_transport_factory,
        input_protocol_factory,
        output_transport_factory,
        output_protocol_factory,
        processor,
        10,
    );

    match server.listen(&listen_addr) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}
