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

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::generated::remote_agent::{AgentException, FileInfo};

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

pub fn read_file(path: &str) -> thrift::Result<Vec<u8>> {
    fs::read(path).map_err(|e| agent_err(format!("Failed to read '{}': {}", path, e)))
}

pub fn write_file(path: &str, data: &[u8]) -> thrift::Result<()> {
    fs::write(path, data).map_err(|e| agent_err(format!("Failed to write '{}': {}", path, e)))
}

pub fn delete_file(path: &str) -> thrift::Result<bool> {
    let p = Path::new(path);
    if !p.exists() {
        return Ok(false);
    }
    if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
    .map(|()| true)
    .map_err(|e| agent_err(format!("Failed to delete '{}': {}", path, e)))
}

pub fn list_directory(path: &str) -> thrift::Result<Vec<FileInfo>> {
    let entries =
        fs::read_dir(path).map_err(|e| agent_err(format!("Failed to list '{}': {}", path, e)))?;

    let mut result = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| agent_err(format!("Error reading entry in '{}': {}", path, e)))?;
        let metadata = entry.metadata().map_err(|e| {
            agent_err(format!(
                "Failed to get metadata for '{}': {}",
                entry.path().display(),
                e
            ))
        })?;

        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);

        let entry_path = entry.path();
        let is_hidden = is_hidden(&entry_path, &metadata);
        let (readable, writable, executable) = permissions(&metadata);
        let user_name = get_owner(&entry_path);

        result.push(FileInfo::new(
            entry.file_name().to_string_lossy().into_owned(),
            entry_path.to_string_lossy().into_owned(),
            metadata.len() as i64,
            last_modified,
            metadata.is_dir(),
            metadata.is_symlink(),
            is_hidden,
            readable,
            writable,
            executable,
            user_name,
        ));
    }

    Ok(result)
}

pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn list_roots() -> Vec<FileInfo> {
    #[cfg(windows)]
    {
        // Check drives A-Z
        let mut roots = Vec::new();
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = Path::new(&drive);
            if path.exists() {
                let metadata = fs::metadata(path).ok();
                let (readable, writable, executable) = metadata
                    .as_ref()
                    .map(|m| permissions(m))
                    .unwrap_or((true, true, true));
                roots.push(FileInfo::new(
                    drive.clone(),
                    drive,
                    0i64,
                    None::<i64>,
                    true,
                    false,
                    false,
                    readable,
                    writable,
                    executable,
                    None::<String>,
                ));
            }
        }
        roots
    }
    #[cfg(not(windows))]
    {
        let root = Path::new("/");
        let metadata = fs::metadata(root).ok();
        let (readable, writable, executable) = metadata
            .as_ref()
            .map(|m| permissions(m))
            .unwrap_or((true, true, true));
        vec![FileInfo::new(
            "/".to_string(),
            "/".to_string(),
            0i64,
            None::<i64>,
            true,
            false,
            false,
            readable,
            writable,
            executable,
            get_owner(root),
        )]
    }
}

pub fn create_directory(path: &str, recursive: bool) -> thrift::Result<()> {
    let result = if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    };
    result.map_err(|e| agent_err(format!("Failed to create directory '{}': {}", path, e)))
}

// --- Platform-specific helpers ---

#[cfg(windows)]
fn is_hidden(_path: &Path, metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(path: &Path, _metadata: &fs::Metadata) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(unix)]
fn permissions(metadata: &fs::Metadata) -> (bool, bool, bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode();
    let readable = mode & 0o444 != 0;
    let writable = mode & 0o222 != 0;
    let executable = mode & 0o111 != 0;
    (readable, writable, executable)
}

#[cfg(windows)]
fn permissions(metadata: &fs::Metadata) -> (bool, bool, bool) {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
    let readonly = metadata.file_attributes() & FILE_ATTRIBUTE_READONLY != 0;
    // Windows: files are always readable if accessible, executable determined by extension
    (true, !readonly, true)
}

#[cfg(unix)]
fn get_owner(path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = path.symlink_metadata().ok()?;
    let uid = metadata.uid();

    // Try to resolve uid to username via /etc/passwd
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 3 {
            if let Ok(entry_uid) = fields[2].parse::<u32>() {
                if entry_uid == uid {
                    return Some(fields[0].to_string());
                }
            }
        }
    }
    Some(uid.to_string())
}

#[cfg(windows)]
fn get_owner(_path: &Path) -> Option<String> {
    std::env::var("USERNAME").ok()
}
