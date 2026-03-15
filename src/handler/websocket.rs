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
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::generated::remote_agent::{AgentException, WebSocketData, WebSocketMessage};

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

struct MessageBuffer {
    messages: Vec<WebSocketMessage>,
    connected: bool,
}

impl MessageBuffer {
    fn new() -> Self {
        MessageBuffer {
            messages: Vec::new(),
            connected: true,
        }
    }

    fn push_binary(&mut self, data: Vec<u8>) {
        self.messages
            .push(WebSocketMessage::new(Some(data), None::<String>));
    }

    fn push_text(&mut self, text: String) {
        self.messages
            .push(WebSocketMessage::new(None::<Vec<u8>>, Some(text)));
    }

    fn mark_disconnected(&mut self) {
        self.connected = false;
    }

    fn drain(&mut self) -> (Vec<WebSocketMessage>, bool) {
        let messages = std::mem::take(&mut self.messages);
        (messages, self.connected)
    }
}

struct ManagedWebSocket {
    socket: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    buffer: Arc<Mutex<MessageBuffer>>,
}

pub struct WebSocketManager {
    sessions: Mutex<BTreeMap<String, ManagedWebSocket>>,
    counter: Mutex<u64>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        WebSocketManager {
            sessions: Mutex::new(BTreeMap::new()),
            counter: Mutex::new(0),
        }
    }

    fn next_id(&self) -> String {
        let mut counter = self.counter.lock().unwrap();
        *counter += 1;
        format!("ws-{}", *counter)
    }

    pub fn connect(
        &self,
        url: String,
        headers: BTreeMap<String, String>,
    ) -> thrift::Result<String> {
        use tungstenite::http::{header::HeaderName, header::HeaderValue, Request};

        let mut request = Request::builder()
            .uri(&url)
            .body(())
            .map_err(|e| agent_err(format!("Invalid WebSocket URL '{}': {}", url, e)))?;

        for (key, value) in &headers {
            let header_name = HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| agent_err(format!("Invalid header name '{}': {}", key, e)))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|e| agent_err(format!("Invalid header value for '{}': {}", key, e)))?;
            request.headers_mut().insert(header_name, header_value);
        }

        let (ws_socket, _response) = tungstenite::connect(request)
            .map_err(|e| agent_err(format!("WebSocket connect failed for '{}': {}", url, e)))?;

        // Set read timeout so reader thread releases lock periodically
        set_read_timeout(&ws_socket, Duration::from_millis(100));

        let socket = Arc::new(Mutex::new(ws_socket));
        let buffer = Arc::new(Mutex::new(MessageBuffer::new()));

        let reader_socket = Arc::clone(&socket);
        let reader_buffer = Arc::clone(&buffer);
        thread::spawn(move || {
            reader_thread(reader_socket, reader_buffer);
        });

        let id = self.next_id();
        self.sessions.lock().unwrap().insert(
            id.clone(),
            ManagedWebSocket { socket, buffer },
        );

        log::info!("WebSocket connected id={} url={}", id, url);
        Ok(id)
    }

    pub fn read_data(&self, session_id: &str) -> thrift::Result<WebSocketData> {
        let sessions = self.sessions.lock().unwrap();
        let managed = sessions
            .get(session_id)
            .ok_or_else(|| agent_err(format!("WebSocket session not found: {}", session_id)))?;

        let (messages, connected) = managed.buffer.lock().unwrap().drain();
        Ok(WebSocketData::new(messages, connected))
    }

    pub fn send_binary(&self, session_id: &str, data: Vec<u8>) -> thrift::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        let managed = sessions
            .get(session_id)
            .ok_or_else(|| agent_err(format!("WebSocket session not found: {}", session_id)))?;

        managed
            .socket
            .lock()
            .unwrap()
            .send(Message::Binary(data.into()))
            .map_err(|e| agent_err(format!("WebSocket send failed for '{}': {}", session_id, e)))
    }

    pub fn send_text(&self, session_id: &str, text: String) -> thrift::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        let managed = sessions
            .get(session_id)
            .ok_or_else(|| agent_err(format!("WebSocket session not found: {}", session_id)))?;

        managed
            .socket
            .lock()
            .unwrap()
            .send(Message::Text(text.into()))
            .map_err(|e| agent_err(format!("WebSocket send failed for '{}': {}", session_id, e)))
    }

    pub fn close(&self, session_id: &str) -> thrift::Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(managed) = sessions.remove(session_id) {
            let _ = managed.socket.lock().unwrap().close(None);
            log::info!("WebSocket closed id={}", session_id);
        }
        Ok(())
    }
}

fn set_read_timeout(ws: &WebSocket<MaybeTlsStream<TcpStream>>, timeout: Duration) {
    match ws.get_ref() {
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

fn reader_thread(
    socket: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    buffer: Arc<Mutex<MessageBuffer>>,
) {
    loop {
        let msg = {
            let mut ws = socket.lock().unwrap();
            ws.read()
        };

        match msg {
            Ok(Message::Binary(data)) => {
                buffer.lock().unwrap().push_binary(data.into());
            }
            Ok(Message::Text(text)) => {
                buffer.lock().unwrap().push_text(text.to_string());
            }
            Ok(Message::Close(_)) => {
                buffer.lock().unwrap().mark_disconnected();
                break;
            }
            Ok(_) => continue, // Ping/Pong handled by tungstenite
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => {
                buffer.lock().unwrap().mark_disconnected();
                break;
            }
        }
    }
}
