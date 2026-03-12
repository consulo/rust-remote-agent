/*
 * Copyright 2013-2026 consulo.io
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

namespace rs remote_agent
namespace java consulo.platform.remote.agent.protocol

// ============================================================
// Exceptions
// ============================================================

exception AgentException {
    1: required string message
}

// ============================================================
// Process Management
// ============================================================

struct ProcessInfo {
    1: required i64 pid,
    2: required bool alive,
    3: optional string command
}

struct ProcessOutput {
    1: optional binary stdoutData,
    2: optional binary stderrData,
    3: optional i32 exitCode
}

// ============================================================
// File Operations
// ============================================================

struct DownloadInfo {
    1: required string transferId,
    2: required i64 fileSize
}

struct FileInfo {
    1: required string name,
    2: required string path,
    3: required i64 size,
    4: optional i64 lastModified,
    5: required bool directory,
    6: required bool symlink,
    7: required bool hidden,
    8: required bool readable,
    9: required bool writable,
    10: required bool executable,
    11: optional string userName
}

// ============================================================
// System / Environment
// ============================================================

struct SystemInfo {
    1: required string osName,
    2: required string osVersion,
    3: required string arch,
    4: required string hostname,
    5: required i32 cpuCount,
    6: required i64 totalMemory,
    // Console encoding for decoding process output (e.g. "UTF-8", "CP866", "CP1251")
    7: required string consoleEncoding,
    // System locale (e.g. "en_US.UTF-8", "uk_UA.UTF-8")
    8: required string locale
}

struct UserInfo {
    1: required string userName,
    2: required string homePath
}

// ============================================================
// Agent Identity
// ============================================================

struct AgentInfo {
    1: required string agentId,       // e.g. "rust-remote-agent"
    2: required string version        // e.g. "0.1.0"
}

// ============================================================
// Service
// ============================================================

service RemoteAgentService {

    // --- Agent Identity ---

    AgentInfo getAgentInfo(),

    // --- Workspace ---

    // Returns the workspace root directory path.
    string getWorkspacePath(),

    // --- Process Management ---

    ProcessInfo startProcess(
        1: required string command,
        2: required list<string> arguments,
        3: optional string workingDirectory,
        4: optional map<string, string> environment
    ) throws (1: AgentException error),

    // force=false: SIGTERM / Ctrl+C (graceful)
    // force=true:  SIGKILL / TerminateProcess (hard)
    bool killProcess(
        1: required i64 pid,
        2: required bool force
    ) throws (1: AgentException error),

    bool isProcessAlive(
        1: required i64 pid
    ),

    list<ProcessInfo> listProcesses(),

    // Returns new output since last read (streaming poll).
    // ANSI escape codes are preserved as raw bytes.
    ProcessOutput readProcessOutput(
        1: required i64 pid
    ) throws (1: AgentException error),

    // --- File Operations ---

    binary readFile(
        1: required string path
    ) throws (1: AgentException error),

    void writeFile(
        1: required string path,
        2: required binary data
    ) throws (1: AgentException error),

    bool deleteFile(
        1: required string path
    ) throws (1: AgentException error),

    list<FileInfo> listDirectory(
        1: required string path
    ) throws (1: AgentException error),

    bool fileExists(
        1: required string path
    ),

    void createDirectory(
        1: required string path,
        2: required bool recursive
    ) throws (1: AgentException error),

    list<FileInfo> listRoots(),

    // Set POSIX permissions (octal mode, e.g. 0755).
    // On Windows this is a no-op that returns false.
    bool setPermissions(
        1: required string path,
        2: required i32 mode
    ) throws (1: AgentException error),

    // --- File Transfer (chunked) ---

    // Upload: host -> agent
    string beginUpload(
        1: required string path,
        2: required i64 fileSize
    ) throws (1: AgentException error),

    void uploadChunk(
        1: required string transferId,
        2: required binary data
    ) throws (1: AgentException error),

    void finishUpload(
        1: required string transferId
    ) throws (1: AgentException error),

    void cancelUpload(
        1: required string transferId
    ) throws (1: AgentException error),

    // Download: agent -> host
    DownloadInfo beginDownload(
        1: required string path
    ) throws (1: AgentException error),

    binary downloadChunk(
        1: required string transferId,
        2: required i32 chunkSize
    ) throws (1: AgentException error),

    void finishDownload(
        1: required string transferId
    ) throws (1: AgentException error),

    // --- Environment / System Info ---

    string getEnvVariable(
        1: required string name
    ),

    map<string, string> getEnvVariables(),

    SystemInfo getSystemInfo(),

    UserInfo getUserInfo()
}
