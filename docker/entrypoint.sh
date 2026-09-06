#!/bin/sh
set -e

case "$(uname -m)" in
    x86_64)  NODE_ARCH=x64;   GO_ARCH=amd64; JDK_ARCH=x64 ;;
    aarch64) NODE_ARCH=arm64; GO_ARCH=arm64; JDK_ARCH=aarch64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

install_java() {
    echo "Installing Temurin JDK $TOOL_JAVA_VERSION"
    mkdir -p "$JAVA_HOME"
    curl -fsSL "https://api.adoptium.net/v3/binary/latest/${TOOL_JAVA_VERSION}/ga/linux/${JDK_ARCH}/jdk/hotspot/normal/eclipse" \
        | tar -xz -C "$JAVA_HOME" --strip-components=1
}

install_nodejs() {
    echo "Installing Node.js $TOOL_NODEJS_VERSION"
    mkdir -p "$NODEJS_HOME"
    curl -fsSL "https://nodejs.org/dist/v${TOOL_NODEJS_VERSION}/node-v${TOOL_NODEJS_VERSION}-linux-${NODE_ARCH}.tar.xz" \
        | tar -xJ -C "$NODEJS_HOME" --strip-components=1
}

install_go() {
    echo "Installing Go $TOOL_GO_VERSION"
    mkdir -p "$GOROOT"
    curl -fsSL "https://go.dev/dl/go${TOOL_GO_VERSION}.linux-${GO_ARCH}.tar.gz" \
        | tar -xz -C "$GOROOT" --strip-components=1
}

install_rust() {
    echo "Installing Rust $TOOL_RUST_VERSION"
    curl -fsSL https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --profile minimal --default-toolchain "$TOOL_RUST_VERSION"
}

install_dotnet() {
    echo "Installing .NET $TOOL_DOTNET_VERSION"
    curl -fsSL https://dot.net/v1/dotnet-install.sh \
        | bash -s -- --channel "$TOOL_DOTNET_VERSION" --install-dir "$DOTNET_ROOT"
}

[ -d "$JAVA_HOME" ] || install_java

for tool in nodejs go rust dotnet; do
    enabled=$(eval "echo \$TOOL_$(echo "$tool" | tr '[:lower:]' '[:upper:]')")
    [ "$enabled" = "1" ] || continue
    [ -e "$TOOLS_DIR/$tool" ] && continue
    "install_$tool"
done

mkdir -p "$WORKSPACE_DIR"

# clap rejects a repeated flag, so the environment defaults are only added
# for the flags the caller did not pass after the image name
has_arg() {
    needle=$1
    shift
    for arg in "$@"; do
        case "$arg" in "$needle"|"$needle"=*) return 0 ;; esac
    done
    return 1
}

has_arg --host "$@" || set -- --host "$AGENT_HOST" "$@"
has_arg --port "$@" || set -- --port "$AGENT_PORT" "$@"
has_arg --workspace "$@" || set -- --workspace "$WORKSPACE_DIR" "$@"
has_arg --permissions "$@" || set -- --permissions "$AGENT_PERMISSIONS" "$@"

exec remote-agent "$@"
