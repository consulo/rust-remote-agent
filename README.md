# rust-remote-agent

A lightweight remote agent for [Consulo IDE](https://consulo.io), providing process management, file operations, and system info over Apache Thrift.

## Download

Pre-built binaries are published to the [consulo/binaries](https://github.com/consulo/binaries) repository under each platform directory as `remote-agent.tar.gz` (or `.zip` on Windows).

| Platform | Architecture | Download |
|----------|-------------|----------|
| Linux | x86_64 | [tar.gz](https://github.com/consulo/binaries/raw/master/linux-x86_64/remote-agent.tar.gz) |
| Linux | aarch64 | [tar.gz](https://github.com/consulo/binaries/raw/master/linux-aarch64/remote-agent.tar.gz) |
| Linux | riscv64 | [tar.gz](https://github.com/consulo/binaries/raw/master/linux-riscv64/remote-agent.tar.gz) |
| Linux | loongarch64 | [tar.gz](https://github.com/consulo/binaries/raw/master/linux-loongarch64/remote-agent.tar.gz) |
| macOS | x86_64 | [tar.gz](https://github.com/consulo/binaries/raw/master/macos-x86_64/remote-agent.tar.gz) |
| macOS | aarch64 (Apple Silicon) | [tar.gz](https://github.com/consulo/binaries/raw/master/macos-aarch64/remote-agent.tar.gz) |
| Windows | x86_64 | [zip](https://github.com/consulo/binaries/raw/master/windows-x86_64/remote-agent.zip) |
| Windows | aarch64 | [zip](https://github.com/consulo/binaries/raw/master/windows-aarch64/remote-agent.zip) |

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
| `stat` | getStat |

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

## Docker / Podman

Build for a specific architecture:

```bash
# x86_64
docker build --platform linux/amd64 -t consulo/remote-agent:latest image/
# or
podman build --platform linux/amd64 -t consulo/remote-agent:latest image/

# aarch64
docker build --platform linux/arm64 -t consulo/remote-agent:latest image/
# or
podman build --platform linux/arm64 -t consulo/remote-agent:latest image/
```

Run:

```bash
docker run -p 57638:57638 consulo/remote-agent:latest
# or
podman run -p 57638:57638 consulo/remote-agent:latest
```

Mount a local workspace:

```bash
docker run -p 57638:57638 -v /my/workspace:/workspace consulo/remote-agent:latest
# or
podman run -p 57638:57638 -v /my/workspace:/workspace consulo/remote-agent:latest
```

## Java Test Client

Generate Java sources without `javax.annotation` annotations:

```bash
thrift --gen java:generated_annotations=suppress -out java-test/src/main/java thrift/remote_agent.thrift
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

Copyright 2013-2026 [consulo.io](https://consulo.io)
