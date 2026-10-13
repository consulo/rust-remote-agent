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

use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tonic::{Status, Streaming};

use crate::error::{AgentError, AgentResult, io_err};
use crate::proto::write_stream_input::Input;
use crate::proto::{FileContent, WriteStreamInput};

const DEFAULT_CHUNK: usize = 256 * 1024;
const MAX_CHUNK: usize = 4 * 1024 * 1024;

pub type ChunkSender = mpsc::Sender<Result<FileContent, Status>>;

pub async fn open_for_read(path: &str) -> AgentResult<tokio::fs::File> {
    tokio::fs::File::open(path)
        .await
        .map_err(|e| io_err(format!("Failed to read '{}'", path), e))
}

pub async fn read_stream(mut file: tokio::fs::File, path: String, chunk_size: i32, chunks: ChunkSender) {
    let size = if chunk_size > 0 {
        (chunk_size as usize).min(MAX_CHUNK)
    } else {
        DEFAULT_CHUNK
    };
    let mut buffer = vec![0u8; size];
    loop {
        match file.read(&mut buffer).await {
            Ok(0) => return,
            Ok(n) => {
                let chunk = FileContent {
                    data: buffer[..n].to_vec(),
                };
                if chunks.send(Ok(chunk)).await.is_err() {
                    return;
                }
            }
            Err(e) => {
                let status: Status = io_err(format!("Failed to read '{}'", path), e).into();
                let _ = chunks.send(Err(status)).await;
                return;
            }
        }
    }
}

pub async fn write_stream(mut input: Streaming<WriteStreamInput>) -> AgentResult<()> {
    let path = match input.message().await {
        Ok(Some(WriteStreamInput {
            input: Some(Input::Path(path)),
        })) => path,
        Ok(_) => {
            return Err(AgentError::InvalidArgument(
                "The first message of a write stream must be the path".to_string(),
            ));
        }
        Err(status) => return Err(AgentError::Failed(status.message().to_string())),
    };

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp_path = format!("{}.consulo-upload-{}", path, nanos);
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| io_err(format!("Failed to write '{}'", path), e))?;

    let result = copy_chunks(&mut input, &mut file, &path).await;
    drop(file);
    match result {
        Ok(()) => tokio::fs::rename(&tmp_path, &path)
            .await
            .map_err(|e| io_err(format!("Failed to write '{}'", path), e)),
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            Err(e)
        }
    }
}

async fn copy_chunks(input: &mut Streaming<WriteStreamInput>, file: &mut tokio::fs::File, path: &str) -> AgentResult<()> {
    loop {
        match input.message().await {
            Ok(Some(WriteStreamInput {
                input: Some(Input::Data(data)),
            })) => file
                .write_all(&data)
                .await
                .map_err(|e| io_err(format!("Failed to write '{}'", path), e))?,
            Ok(Some(_)) => {}
            Ok(None) => {
                return file
                    .flush()
                    .await
                    .map_err(|e| io_err(format!("Failed to write '{}'", path), e));
            }
            Err(status) => {
                return Err(AgentError::Failed(format!(
                    "Write of '{}' was cancelled: {}",
                    path,
                    status.message()
                )));
            }
        }
    }
}
