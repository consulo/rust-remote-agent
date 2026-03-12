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

#[derive(Parser)]
#[command(name = "remote-agent", about = "Remote agent for Consulo IDE")]
struct Cli {
    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,

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
    log::info!("Starting remote-agent on {} (platform: {})", listen_addr, platform::platform_label());
    println!("remote-agent listening on {} (platform: {})", listen_addr, platform::platform_label());

    let handler = AgentServiceHandler::new();
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
