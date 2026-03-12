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
remote-agent --port 9090
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

Copyright 2013-2026 [consulo.io](https://consulo.io)
