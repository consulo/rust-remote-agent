# rust-remote-agent

A lightweight remote agent for [Consulo IDE](https://consulo.io), providing process management, file operations, and system info over Apache Thrift.

## Download

Pre-built binaries are available on the [Releases](https://github.com/consulo/rust-remote-agent/releases/latest) page.

| Platform | Architecture | Download |
|----------|-------------|----------|
| Linux | x86_64 | [tar.gz](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-unknown-linux-gnu.tar.gz) |
| Linux | aarch64 | [tar.gz](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-unknown-linux-gnu.tar.gz) |
| macOS | x86_64 | [tar.gz](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-apple-darwin.tar.gz) |
| macOS | aarch64 (Apple Silicon) | [tar.gz](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-apple-darwin.tar.gz) |
| Windows | x86_64 | [zip](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-pc-windows-msvc.zip) |
| Windows | aarch64 | [zip](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-pc-windows-msvc.zip) |

## Build

```bash
cargo build --release
```

## Usage

```bash
remote-agent [OPTIONS]
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `--host <HOST>` | Host address to bind to | `0.0.0.0` |
| `--port <PORT>` | Port to listen on | `57638` |
| `--workspace <PATH>` | Workspace root directory | `~/consulo-workspace` |
| `--permissions <GROUPS>` | Comma-separated permission groups or `*` for all | `*` |
| `--help` | Print help | |
| `--version` | Print version | |

### Permission Groups

| Group | Methods |
|-------|---------|
| `fs` | readFile, writeFile, deleteFile, listDirectory, fileExists, createDirectory, listRoots, setPermissions, beginUpload, uploadChunk, finishUpload, cancelUpload, beginDownload, downloadChunk, finishDownload |
| `process` | startProcess, killProcess, isProcessAlive, listProcesses, readProcessOutput |
| `http` | executeHttpRequest |
| `websocket` | connectWebSocket, readWebSocketData, sendWebSocketData, closeWebSocket |
| `userinfo` | getUserInfo |

Methods that are always accessible (no permission needed): `getAgentInfo`, `getWorkspacePath`, `getSystemInfo`, `getEnvVariable`, `getEnvVariables`, `findFreePort`.

### Examples

```bash
# Start with all permissions (default)
remote-agent

# Restrict to file and HTTP operations only
remote-agent --permissions fs,http

# Custom port and workspace
remote-agent --port 9090 --workspace /opt/workspace --permissions fs,process
```

## Java Test Client

Generate Java sources without `javax.annotation` annotations:

```bash
thrift --gen java:generated_annotations=suppress -out java-test/src/main/java thrift/remote_agent.thrift
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

Copyright 2013-2026 [consulo.io](https://consulo.io)
