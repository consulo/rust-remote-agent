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

mod error;
mod handler;
mod platform;
mod security;
mod service;

pub mod proto {
    tonic::include_proto!("remote_agent");
}

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use clap::Parser;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Identity, Server, ServerTlsConfig};

use crate::proto::remote_agent_service_server::RemoteAgentServiceServer;
use crate::security::TokenCheck;
use crate::service::AgentService;

const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

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
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to listen on
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Workspace root directory
    #[arg(long, default_value_t = default_workspace())]
    workspace: String,

    /// Comma-separated permission groups: fs,process,http,websocket,userinfo,stat or * for all
    #[arg(long, default_value = "*")]
    permissions: String,

    /// Serve over TLS; without --tls-cert/--tls-key a self-signed certificate is created in the workspace
    #[arg(long, env = "AGENT_TLS")]
    tls: bool,

    /// PEM certificate (chain) for TLS
    #[arg(long, env = "AGENT_TLS_CERT", requires = "tls_key")]
    tls_cert: Option<PathBuf>,

    /// PEM private key for TLS
    #[arg(long, env = "AGENT_TLS_KEY", requires = "tls_cert")]
    tls_key: Option<PathBuf>,

    /// Token clients must send as "authorization: Bearer <token>"
    #[arg(long, env = "AGENT_TOKEN", hide_env_values = true)]
    token: Option<String>,
}

/// Detect the local LAN IP by opening a UDP socket to a public address.
/// No actual traffic is sent — the OS just picks the right local interface.
fn detect_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn describe_bind_error(error: &std::io::Error, addr: SocketAddr) -> String {
    let port = addr.port();
    match error.kind() {
        std::io::ErrorKind::AddrInUse => format!(
            "Cannot listen on {addr}: port {port} is already used by another program \
             (maybe another remote-agent or a container publishing it).\n\
             Stop that program or choose another port with --port <PORT>. \
             To see who holds it: ss -ltnp 'sport = :{port}'"
        ),
        std::io::ErrorKind::PermissionDenied => format!(
            "Cannot listen on {addr}: permission denied. Ports below 1024 need root; use --port with a higher port."
        ),
        std::io::ErrorKind::AddrNotAvailable => format!(
            "Cannot listen on {addr}: the address {} does not belong to this machine. \
             Use --host 0.0.0.0 to listen on all interfaces.",
            addr.ip()
        ),
        _ => format!("Cannot listen on {addr}: {error}"),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    let listen_addr = format!("{}:{}", cli.host, cli.port);
    let socket_addr: SocketAddr = match listen_addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Invalid listen address '{}': {}", listen_addr, e);
            std::process::exit(1);
        }
    };
    let workspace = cli.workspace;
    let permissions = service::Permissions::parse(&cli.permissions);

    // Ensure workspace directory exists
    if let Err(e) = std::fs::create_dir_all(&workspace) {
        eprintln!("Failed to create workspace '{}': {}", workspace, e);
        std::process::exit(1);
    }

    let local_ip = detect_local_ip().unwrap_or_else(|| "unknown".to_string());

    let tls = if cli.tls || cli.tls_cert.is_some() {
        let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string(), handler::env::get_system_info().hostname];
        if local_ip != "unknown" {
            names.push(local_ip.clone());
        }
        let state_dir = Path::new(&workspace).join(".remote-agent");
        match security::load_tls(cli.tls_cert.as_deref(), cli.tls_key.as_deref(), &state_dir, names) {
            Ok(material) => Some(material),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };
    let token = cli.token.filter(|t| !t.is_empty());

    let listener = match tokio::net::TcpListener::bind(socket_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("{}", describe_bind_error(&e, socket_addr));
            std::process::exit(1);
        }
    };

    log::info!("Starting remote-agent on {} (platform: {}, workspace: {})", listen_addr, platform::platform_label(), workspace);
    println!("Consulo Remote Agent v{}", env!("CARGO_PKG_VERSION"));
    println!("platform: {}", platform::platform_label());
    println!("workspace: {}", workspace);
    println!("permissions: {}", permissions.display());
    match &tls {
        Some(material) => {
            println!("tls: on, certificate SHA-256 {}", material.fingerprint);
            if let Some(dir) = &material.generated_at {
                println!("tls: self-signed certificate in {}", dir.display());
            }
        }
        None => println!("tls: off"),
    }
    println!("token: {}", if token.is_some() { "required" } else { "not required" });
    if token.is_none() {
        println!("warning: no --token, anyone who can reach the port can use this agent");
    }


    let service = RemoteAgentServiceServer::new(AgentService::new(workspace, permissions))
        .max_decoding_message_size(MAX_MESSAGE_SIZE)
        .max_encoding_message_size(MAX_MESSAGE_SIZE);
    let service = InterceptedService::new(service, TokenCheck::new(token));

    let mut builder = Server::builder();
    if let Some(material) = tls {
        let config = ServerTlsConfig::new().identity(Identity::from_pem(material.cert_pem, material.key_pem));
        builder = match builder.tls_config(config) {
            Ok(builder) => builder,
            Err(e) => {
                eprintln!("Invalid TLS configuration: {}", e);
                std::process::exit(1);
            }
        };
    }

    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    println!("listening on {}, connect via {}:{}", listen_addr, local_ip, cli.port);
    {
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    let result = builder.add_service(service).serve_with_incoming_shutdown(incoming, shutdown_signal()).await;
    if let Err(e) = result {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    log::info!("Shutting down");
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    log::info!("Shutting down");
}
