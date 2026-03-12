# rust-remote-agent

A lightweight remote agent for [Consulo IDE](https://consulo.io), providing process management, file operations, and system info over Apache Thrift.

## Download

Pre-built binaries are available on the [Releases](https://github.com/consulo/rust-remote-agent/releases/latest) page.

| Platform | Architecture | Download |
|----------|-------------|----------|
| Linux | x86_64 | [remote-agent-x86_64-unknown-linux-gnu](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-unknown-linux-gnu) |
| Linux | aarch64 | [remote-agent-aarch64-unknown-linux-gnu](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-unknown-linux-gnu) |
| macOS | x86_64 | [remote-agent-x86_64-apple-darwin](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-apple-darwin) |
| macOS | aarch64 (Apple Silicon) | [remote-agent-aarch64-apple-darwin](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-apple-darwin) |
| Windows | x86_64 | [remote-agent-x86_64-pc-windows-msvc.exe](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-x86_64-pc-windows-msvc.exe) |
| Windows | aarch64 | [remote-agent-aarch64-pc-windows-msvc.exe](https://github.com/consulo/rust-remote-agent/releases/latest/download/remote-agent-aarch64-pc-windows-msvc.exe) |

## Build

```bash
cargo build --release
```

## Usage

```bash
remote-agent --port 9090
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

Copyright 2013-2026 [consulo.io](https://consulo.io)
