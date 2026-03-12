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
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::sync::Mutex;

use crate::generated::remote_agent::{AgentException, DownloadInfo};

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

struct Upload {
    file: File,
    path: String,
    tmp_path: String,
}

struct Download {
    file: File,
    remaining: i64,
}

pub struct TransferManager {
    uploads: Mutex<BTreeMap<String, Upload>>,
    downloads: Mutex<BTreeMap<String, Download>>,
    counter: Mutex<u64>,
}

impl TransferManager {
    pub fn new() -> Self {
        TransferManager {
            uploads: Mutex::new(BTreeMap::new()),
            downloads: Mutex::new(BTreeMap::new()),
            counter: Mutex::new(0),
        }
    }

    fn next_id(&self, prefix: &str) -> String {
        let mut counter = self.counter.lock().unwrap();
        *counter += 1;
        format!("{}-{}", prefix, *counter)
    }

    // --- Upload: host -> agent ---

    pub fn begin_upload(&self, path: String, _file_size: i64) -> thrift::Result<String> {
        let tmp_path = format!("{}.tmp", &path);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| agent_err(format!("Failed to create '{}': {}", tmp_path, e)))?;

        let id = self.next_id("up");
        self.uploads.lock().unwrap().insert(
            id.clone(),
            Upload {
                file,
                path,
                tmp_path,
            },
        );

        log::info!("Begin upload id={}", id);
        Ok(id)
    }

    pub fn upload_chunk(&self, transfer_id: &str, data: &[u8]) -> thrift::Result<()> {
        let mut uploads = self.uploads.lock().unwrap();
        let upload = uploads
            .get_mut(transfer_id)
            .ok_or_else(|| agent_err(format!("Unknown upload: {}", transfer_id)))?;

        upload
            .file
            .write_all(data)
            .map_err(|e| agent_err(format!("Write failed for upload {}: {}", transfer_id, e)))
    }

    pub fn finish_upload(&self, transfer_id: &str) -> thrift::Result<()> {
        let mut uploads = self.uploads.lock().unwrap();
        let upload = uploads
            .remove(transfer_id)
            .ok_or_else(|| agent_err(format!("Unknown upload: {}", transfer_id)))?;

        drop(upload.file);

        fs::rename(&upload.tmp_path, &upload.path).map_err(|e| {
            agent_err(format!(
                "Failed to rename '{}' -> '{}': {}",
                upload.tmp_path, upload.path, e
            ))
        })?;

        log::info!("Finished upload id={} path={}", transfer_id, upload.path);
        Ok(())
    }

    pub fn cancel_upload(&self, transfer_id: &str) -> thrift::Result<()> {
        let mut uploads = self.uploads.lock().unwrap();
        if let Some(upload) = uploads.remove(transfer_id) {
            drop(upload.file);
            let _ = fs::remove_file(&upload.tmp_path);
            log::info!("Cancelled upload id={}", transfer_id);
        }
        Ok(())
    }

    // --- Download: agent -> host ---

    pub fn begin_download(&self, path: &str) -> thrift::Result<DownloadInfo> {
        let metadata = fs::metadata(path)
            .map_err(|e| agent_err(format!("Failed to stat '{}': {}", path, e)))?;

        let file_size = metadata.len() as i64;

        let file = File::open(path)
            .map_err(|e| agent_err(format!("Failed to open '{}': {}", path, e)))?;

        let id = self.next_id("dl");
        self.downloads.lock().unwrap().insert(
            id.clone(),
            Download {
                file,
                remaining: file_size,
            },
        );

        log::info!("Begin download id={} path={} size={}", id, path, file_size);
        Ok(DownloadInfo::new(id, file_size))
    }

    pub fn download_chunk(
        &self,
        transfer_id: &str,
        chunk_size: i32,
    ) -> thrift::Result<Vec<u8>> {
        let mut downloads = self.downloads.lock().unwrap();
        let download = downloads
            .get_mut(transfer_id)
            .ok_or_else(|| agent_err(format!("Unknown download: {}", transfer_id)))?;

        let to_read = (chunk_size as i64).min(download.remaining) as usize;
        let mut buf = vec![0u8; to_read];
        let n = download
            .file
            .read(&mut buf)
            .map_err(|e| agent_err(format!("Read failed for download {}: {}", transfer_id, e)))?;

        buf.truncate(n);
        download.remaining -= n as i64;
        Ok(buf)
    }

    pub fn finish_download(&self, transfer_id: &str) -> thrift::Result<()> {
        self.downloads.lock().unwrap().remove(transfer_id);
        log::info!("Finished download id={}", transfer_id);
        Ok(())
    }
}
