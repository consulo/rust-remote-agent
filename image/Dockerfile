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

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=downloader /usr/local/bin/remote-agent /usr/local/bin/remote-agent

WORKDIR /workspace

VOLUME ["/workspace"]

EXPOSE 57638

ENTRYPOINT ["remote-agent", "--workspace", "/workspace"]
