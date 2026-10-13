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

use std::net::TcpStream;
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::Duration;

use tokio::sync::mpsc;
use tonic::{Status, Streaming};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::{HeaderName, HeaderValue};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::error::{AgentError, AgentResult, agent_err};
use crate::proto::web_socket_event::Event;
use crate::proto::web_socket_input::Input;
use crate::proto::{Empty, WebSocketConnect, WebSocketEvent, WebSocketInput};

pub type Socket = WebSocket<MaybeTlsStream<TcpStream>>;
pub type EventSender = mpsc::Sender<Result<WebSocketEvent, Status>>;

const READ_TIMEOUT: Duration = Duration::from_millis(50);

enum Outgoing {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

/// Opens the socket: the handshake headers come from the URL, the caller's headers are added on top.
pub fn connect(request: WebSocketConnect) -> AgentResult<Socket> {
    let url = request.url;
    let mut client_request = url
        .as_str()
        .into_client_request()
        .map_err(|e| AgentError::InvalidArgument(format!("Invalid WebSocket URL '{}': {}", url, e)))?;

    for (key, value) in &request.headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| AgentError::InvalidArgument(format!("Invalid header name '{}': {}", key, e)))?;
        let value = HeaderValue::from_str(value)
            .map_err(|e| AgentError::InvalidArgument(format!("Invalid header value for '{}': {}", key, e)))?;
        client_request.headers_mut().insert(name, value);
    }

    let (socket, _response) = tungstenite::connect(client_request)
        .map_err(|e| agent_err(format!("WebSocket connect failed for '{}': {}", url, e)))?;
    set_read_timeout(&socket, READ_TIMEOUT);
    log::info!("WebSocket connected url={}", url);
    Ok(socket)
}

/// Drives an open socket until either side closes it.
pub fn run(socket: Socket, input: Streaming<WebSocketInput>, events: EventSender) {
    let (outgoing_tx, outgoing_rx) = std_mpsc::channel();
    tokio::spawn(feed_input(input, outgoing_tx));

    let _ = events.try_send(Ok(WebSocketEvent {
        event: Some(Event::Opened(Empty {})),
    }));
    thread::spawn(move || pump(socket, outgoing_rx, events));
}

async fn feed_input(mut input: Streaming<WebSocketInput>, outgoing: std_mpsc::Sender<Outgoing>) {
    loop {
        let next = match input.message().await {
            Ok(Some(message)) => match message.input {
                Some(Input::Text(text)) => Outgoing::Text(text),
                Some(Input::Binary(data)) => Outgoing::Binary(data),
                Some(Input::Connect(_)) | None => continue,
            },
            Ok(None) | Err(_) => Outgoing::Close,
        };
        let close = matches!(next, Outgoing::Close);
        if outgoing.send(next).is_err() || close {
            return;
        }
    }
}

fn pump(mut socket: Socket, outgoing: std_mpsc::Receiver<Outgoing>, events: EventSender) {
    loop {
        loop {
            match outgoing.try_recv() {
                Ok(Outgoing::Text(text)) => {
                    if socket.send(Message::Text(text.into())).is_err() {
                        return;
                    }
                }
                Ok(Outgoing::Binary(data)) => {
                    if socket.send(Message::Binary(data.into())).is_err() {
                        return;
                    }
                }
                Ok(Outgoing::Close) | Err(std_mpsc::TryRecvError::Disconnected) => {
                    let _ = socket.close(None);
                    let _ = socket.flush();
                    return;
                }
                Err(std_mpsc::TryRecvError::Empty) => break,
            }
        }

        let event = match socket.read() {
            Ok(Message::Text(text)) => Event::Text(text.to_string()),
            Ok(Message::Binary(data)) => Event::Binary(data.into()),
            Ok(Message::Close(_)) => return,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                log::debug!("WebSocket closed: {}", e);
                return;
            }
        };
        if events.blocking_send(Ok(WebSocketEvent { event: Some(event) })).is_err() {
            let _ = socket.close(None);
            return;
        }
    }
}

fn set_read_timeout(socket: &Socket, timeout: Duration) {
    match socket.get_ref() {
        MaybeTlsStream::Plain(tcp) => {
            let _ = tcp.set_read_timeout(Some(timeout));
        }
        MaybeTlsStream::Rustls(tls) => {
            let _ = tls.get_ref().set_read_timeout(Some(timeout));
        }
        _ => {
            log::warn!("Cannot set read timeout for this stream type");
        }
    }
}
