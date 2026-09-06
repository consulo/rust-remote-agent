FROM debian:bookworm-slim AS downloader

ARG TARGETARCH

RUN apt-get update && \
    apt-get install -y --no-install-recommends curl ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    case "${TARGETARCH}" in \
      "amd64")    ARCH="linux-x86_64" ;; \
      "arm64")    ARCH="linux-aarch64" ;; \
      "riscv64")  ARCH="linux-riscv64" ;; \
      "loong64")  ARCH="linux-loongarch64" ;; \
      *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac && \
    curl -fL "https://github.com/consulo/binaries/raw/master/${ARCH}/remote-agent.tar.gz" \
    | tar xz -C /usr/local/bin

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    JAVA_HOME=/opt/tools/jdk \
    PATH=/opt/tools/jdk/bin:/opt/tools/nodejs/bin:/opt/tools/go/bin:/root/go/bin:/opt/tools/dotnet:/opt/tools/rust/cargo/bin:$PATH \
    TOOL_JAVA_VERSION=25 \
    AGENT_HOST=0.0.0.0 \
    AGENT_PORT=57638 \
    AGENT_PERMISSIONS=* \
    WORKSPACE_DIR=/workspace \
    TOOLS_DIR=/opt/tools \
    NODEJS_HOME=/opt/tools/nodejs \
    GOROOT=/opt/tools/go \
    GOPATH=/root/go \
    DOTNET_ROOT=/opt/tools/dotnet \
    RUSTUP_HOME=/opt/tools/rust/rustup \
    CARGO_HOME=/opt/tools/rust/cargo \
    TOOL_NODEJS=1 \
    TOOL_NODEJS_VERSION=24.19.0 \
    TOOL_GO=1 \
    TOOL_GO_VERSION=1.26.5 \
    TOOL_RUST=1 \
    TOOL_RUST_VERSION=stable \
    TOOL_DOTNET=1 \
    TOOL_DOTNET_VERSION=LTS

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        libicu74 \
        nano \
        tar \
        xz-utils && \
    rm -rf /var/lib/apt/lists/*

COPY --from=downloader /usr/local/bin/remote-agent /usr/local/bin/remote-agent
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/remote-agent

WORKDIR /workspace

VOLUME /workspace
VOLUME /opt/tools

EXPOSE 57638

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
