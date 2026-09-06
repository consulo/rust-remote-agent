# Consulo Remote Agent

**A lightweight remote agent for [Consulo IDE](https://consulo.io).** It exposes process
management, file operations, HTTP/WebSocket proxying and system info over
[Apache Thrift](https://thrift.apache.org), so a Consulo instance running elsewhere can use this
container as its execution environment.

The agent is written in Rust and developed in the open at
[github.com/consulo/rust-remote-agent](https://github.com/consulo/rust-remote-agent), under the
Apache-2.0 license.

## Quick start

```bash
docker run --rm -p 127.0.0.1:57638:57638 consuloide/remote-agent:latest
```

The agent listens on `57638` and serves `/workspace` as the workspace root. Mount a volume there
to keep the files across restarts:

```bash
docker run --rm -p 127.0.0.1:57638:57638 \
    -v /my/workspace:/workspace \
    consuloide/remote-agent:latest
```

## Tags

- `latest` — built from the current `master` binaries

Multi-arch: `linux/amd64` and `linux/arm64`.

## Contents

- Ubuntu 24.04
- `remote-agent`, built from `master` and published to
  [consulo/binaries](https://github.com/consulo/binaries)
- Eclipse Temurin JDK, installed at first start into `/opt/tools/jdk` (`JAVA_HOME`)
- `git`, `nano`, `curl`, `ca-certificates`

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `AGENT_HOST` | host the agent binds to | `0.0.0.0` |
| `AGENT_PORT` | port the agent listens on | `57638` |
| `AGENT_PERMISSIONS` | enabled permission groups, or `*` for all | `*` |
| `WORKSPACE_DIR` | workspace root | `/workspace` |

Permission groups are `fs`, `process`, `http`, `websocket`, `userinfo` and `stat`, given as a
comma-separated list. `getAgentInfo`, `getWorkspacePath`, `getSystemInfo`, `getEnvVariable`,
`getEnvVariables` and `findFreePort` are always available.

Arguments after the image name go to the agent, and a flag given there replaces the matching
environment default:

```bash
docker run --rm -p 127.0.0.1:9090:9090 \
    consuloide/remote-agent:latest --port 9090 --permissions fs,process
```

## Optional toolchains

Node.js, Go, Rust and .NET are installed into `/opt/tools` on startup and put on `PATH`. Set
`TOOL_NODEJS`, `TOOL_GO`, `TOOL_RUST` or `TOOL_DOTNET` to `0` to skip one; mount a volume at
`/opt/tools` to install them only once.

```bash
docker run --rm -p 127.0.0.1:57638:57638 \
    -v /my/workspace:/workspace -v remote-agent-tools:/opt/tools \
    -e TOOL_DOTNET=0 \
    consuloide/remote-agent:latest
```

Versions are pinned with `TOOL_JAVA_VERSION`, `TOOL_NODEJS_VERSION`, `TOOL_GO_VERSION`,
`TOOL_RUST_VERSION` and `TOOL_DOTNET_VERSION`.

## Links

- Website — <https://consulo.io>
- Sources for this image — <https://github.com/consulo/rust-remote-agent>
- Platform sources — <https://github.com/consulo/consulo>
- Issues — <https://github.com/consulo/consulo/issues>
