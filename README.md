# rust-remote-agent

A lightweight remote agent for [Consulo IDE](https://consulo.io), providing process management, file operations, HTTP/WebSocket proxying and system info over [gRPC](https://grpc.io).

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
| `fs` | ReadFile, WriteFile, ReadFileStream, WriteFileStream, DeleteFile, ListDirectory, FileExists, CreateDirectory, ListRoots, SetPermissions |
| `process` | Exec, KillProcess, ListProcesses |
| `http` | ExecuteHttpRequest |
| `websocket` | WebSocket |
| `userinfo` | GetUserInfo |
| `stat` | GetStat |

Methods that are always accessible (no permission needed): `GetAgentInfo`, `GetWorkspacePath`, `GetSystemInfo`, `GetEnvVariable`, `GetEnvVariables`, `FindFreePort`.

A call without the permission fails with `PERMISSION_DENIED`; the denied group is in the `permission` trailer.

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

Published as [`consuloide/remote-agent`](https://hub.docker.com/r/consuloide/remote-agent) for `linux/amd64` and `linux/arm64`.

```bash
docker run --pull=always -p 127.0.0.1:57638:57638 docker.io/consuloide/remote-agent:latest
# or
podman run --pull=always -p 127.0.0.1:57638:57638 docker.io/consuloide/remote-agent:latest
```

Mount a local workspace, and keep the downloaded toolchains between runs:

```bash
docker run -p 127.0.0.1:57638:57638 \
  -v /my/workspace:/workspace \
  -v remote-agent-tools:/opt/tools \
  docker.io/consuloide/remote-agent:latest
```

The image ships the agent only - the toolchains are downloaded into `/opt/tools` on first
start by the entrypoint, and are skipped when already present:

| Variable | Description | Default |
|----------|-------------|---------|
| `TOOL_JAVA_VERSION` | Temurin JDK version (always installed) | `25` |
| `TOOL_NODEJS` / `TOOL_NODEJS_VERSION` | install Node.js / version | `1` / `24.19.0` |
| `TOOL_GO` / `TOOL_GO_VERSION` | install Go / version | `1` / `1.26.5` |
| `TOOL_RUST` / `TOOL_RUST_VERSION` | install Rust / toolchain | `1` / `stable` |
| `TOOL_DOTNET` / `TOOL_DOTNET_VERSION` | install .NET / channel | `1` / `LTS` |
| `AGENT_HOST` | host passed to the agent | `0.0.0.0` |
| `AGENT_PORT` | port passed to the agent | `57638` |
| `AGENT_PERMISSIONS` | permission groups passed to the agent | `*` |
| `WORKSPACE_DIR` | workspace passed to the agent | `/workspace` |

Set `TOOL_<NAME>=0` to skip a toolchain, e.g. `-e TOOL_DOTNET=0 -e TOOL_GO=0`. Arguments
given after the image name are passed to the agent, and a flag given there replaces the
matching environment default:

```bash
docker run -p 127.0.0.1:9090:9090 docker.io/consuloide/remote-agent:latest --port 9090
```

Build locally for a specific architecture - the image pulls the binary published to
[consulo/binaries](https://github.com/consulo/binaries):

```bash
# x86_64
docker build --platform linux/amd64 -t consuloide/remote-agent:latest docker/
# or
podman build --platform linux/amd64 -t consuloide/remote-agent:latest docker/

# aarch64
docker build --platform linux/arm64 -t consuloide/remote-agent:latest docker/
# or
podman build --platform linux/arm64 -t consuloide/remote-agent:latest docker/
```

## Protocol

The service is defined in [`proto/remote_agent.proto`](proto/remote_agent.proto). Long-lived
interactions are streams: `Exec` (process output, stdin, kill), `WebSocket`, `ReadFileStream` and
`WriteFileStream`. Errors are gRPC statuses (`NOT_FOUND`, `ALREADY_EXISTS`, `PERMISSION_DENIED`,
`INVALID_ARGUMENT`, `UNKNOWN`). The connection is plaintext HTTP/2 - bind to `127.0.0.1` or
tunnel it (SSH, `docker exec`) on untrusted networks.

`protoc` comes from the `protoc-bin-vendored` crate, so building needs only `cargo`.

## Java Test Client

[`java-test`](java-test) generates its gRPC stubs from the same proto and checks every call
against a running agent (exit code 1 on the first failure):

```bash
cd java-test
mvn package dependency:build-classpath -Dmdep.outputFile=target/classpath.txt
java -cp "target/classes:$(cat target/classpath.txt)" consulo.platform.remote.agent.test.TestClient 127.0.0.1 57638
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

Copyright 2013-2026 [consulo.io](https://consulo.io)
