#!/usr/bin/env bash
# FieldMid CLI installer
# Usage: curl -fsSL https://fieldmid.com/install.sh | sh
#
# This script downloads the pre-built fieldmid binary for your platform
# and installs it to /usr/local/bin (or ~/.local/bin if no sudo).

set -euo pipefail

REPO="fieldmid/rust-edge-repo"
BINARY_NAME="fieldmid"
INSTALL_DIR="/usr/local/bin"

# ─── Helpers ──────────────────────────────────────────────────────────────

info()  { printf "\033[1m%s\033[0m\n" "$*"; }
ok()    { printf "  \033[32m✓\033[0m %s\n" "$*"; }
warn()  { printf "  \033[33m⚠\033[0m %s\n" "$*"; }
fail()  { printf "  \033[31m✗\033[0m %s\n" "$*"; exit 1; }

# ─── Detect platform ─────────────────────────────────────────────────────

detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)   os="linux" ;;
        Darwin)  os="darwin" ;;
        *)       fail "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        armv7l)         arch="armv7" ;;
        *)              fail "Unsupported architecture: $arch" ;;
    esac

    echo "${os}-${arch}"
}

# ─── Detect latest version ───────────────────────────────────────────────

detect_version() {
    local version
    if command -v curl >/dev/null 2>&1; then
        version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
            | grep '"tag_name"' | head -1 | sed 's/.*"tag_name":\s*"//;s/".*//')
    elif command -v wget >/dev/null 2>&1; then
        version=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
            | grep '"tag_name"' | head -1 | sed 's/.*"tag_name":\s*"//;s/".*//')
    fi

    if [ -z "${version:-}" ]; then
        # Fallback: build from source
        echo ""
    else
        echo "$version"
    fi
}

# ─── Download binary ─────────────────────────────────────────────────────

download_binary() {
    local platform="$1"
    local version="$2"
    local url="https://github.com/${REPO}/releases/download/${version}/fieldmid-${platform}"
    local tmpdir

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    info "Downloading fieldmid ${version} for ${platform}..."

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "${tmpdir}/${BINARY_NAME}" "$url" || return 1
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${tmpdir}/${BINARY_NAME}" "$url" || return 1
    else
        fail "Neither curl nor wget found"
    fi

    chmod +x "${tmpdir}/${BINARY_NAME}"
    echo "${tmpdir}/${BINARY_NAME}"
}

# ─── Build from source ───────────────────────────────────────────────────

build_from_source() {
    info "No pre-built binary found. Building from source..."

    if ! command -v cargo >/dev/null 2>&1; then
        warn "Rust is not installed."
        info ""
        info "Install Rust first:"
        info "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        info ""
        info "Then install fieldmid:"
        info "  cargo install --git https://github.com/${REPO}.git"
        exit 1
    fi

    cargo install --git "https://github.com/${REPO}.git" 2>&1
    ok "Built and installed via cargo"
    return 0
}

# ─── Install ─────────────────────────────────────────────────────────────

install_binary() {
    local src="$1"

    # Try /usr/local/bin with sudo, fall back to ~/.local/bin
    if [ -w "${INSTALL_DIR}" ]; then
        cp "$src" "${INSTALL_DIR}/${BINARY_NAME}"
        ok "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
    elif command -v sudo >/dev/null 2>&1; then
        sudo cp "$src" "${INSTALL_DIR}/${BINARY_NAME}"
        ok "Installed to ${INSTALL_DIR}/${BINARY_NAME} (via sudo)"
    else
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "${INSTALL_DIR}"
        cp "$src" "${INSTALL_DIR}/${BINARY_NAME}"
        ok "Installed to ${INSTALL_DIR}/${BINARY_NAME}"

        # Check if ~/.local/bin is in PATH
        if ! echo "$PATH" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
            warn "Add ${INSTALL_DIR} to your PATH:"
            info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        fi
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────────

main() {
    echo ""
    info "FieldMid CLI Installer"
    echo ""

    local platform version

    platform="$(detect_platform)"
    ok "Platform: ${platform}"

    version="$(detect_version)"

    if [ -n "$version" ]; then
        ok "Latest version: ${version}"

        local binary_path
        binary_path="$(download_binary "$platform" "$version" 2>/dev/null)" || binary_path=""

        if [ -n "$binary_path" ] && [ -f "$binary_path" ]; then
            install_binary "$binary_path"
        else
            warn "Pre-built binary not available for ${platform}"
            build_from_source
        fi
    else
        build_from_source
    fi

    echo ""
    info "Getting started:"
    echo "  fieldmid login              # Authenticate"
    echo "  fieldmid                    # Start the edge daemon"
    echo "  fieldmid latest-incidents   # View incidents"
    echo "  fieldmid requests           # View and approve join requests"
    echo "  fieldmid help               # All commands"
    echo ""
}

main "$@"
