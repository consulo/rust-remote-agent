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

use std::fmt;
use std::io;

use tonic::metadata::MetadataValue;
use tonic::{Code, Status};

#[derive(Debug)]
pub enum AgentError {
    Failed(String),
    Permission { message: String, permission: String },
    NotFound(String),
    AlreadyExists(String),
    InvalidArgument(String),
}

pub type AgentResult<T> = Result<T, AgentError>;

pub fn agent_err(message: String) -> AgentError {
    AgentError::Failed(message)
}

/// Maps an I/O error to the matching status, keeping the context in the message.
pub fn io_err(context: String, error: io::Error) -> AgentError {
    let message = format!("{}: {}", context, error);
    match error.kind() {
        io::ErrorKind::NotFound => AgentError::NotFound(message),
        io::ErrorKind::AlreadyExists => AgentError::AlreadyExists(message),
        io::ErrorKind::PermissionDenied => AgentError::Permission {
            message,
            permission: "os".to_string(),
        },
        _ => AgentError::Failed(message),
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Failed(m)
            | AgentError::NotFound(m)
            | AgentError::AlreadyExists(m)
            | AgentError::InvalidArgument(m) => f.write_str(m),
            AgentError::Permission { message, .. } => f.write_str(message),
        }
    }
}

impl From<AgentError> for Status {
    fn from(error: AgentError) -> Self {
        match error {
            AgentError::Failed(m) => Status::new(Code::Unknown, m),
            AgentError::NotFound(m) => Status::new(Code::NotFound, m),
            AgentError::AlreadyExists(m) => Status::new(Code::AlreadyExists, m),
            AgentError::InvalidArgument(m) => Status::new(Code::InvalidArgument, m),
            AgentError::Permission { message, permission } => {
                let mut status = Status::new(Code::PermissionDenied, message);
                if let Ok(value) = MetadataValue::try_from(permission.as_str()) {
                    status.metadata_mut().insert("permission", value);
                }
                status
            }
        }
    }
}
