#!/bin/bash

set -euo pipefail

# Detect OS
OS=$(uname -s)
case "${OS}" in
  Linux)
    ;;
  Darwin)
    ;;
  *)
    echo "Error: Unsupported OS: ${OS}"
    exit 1
    ;;
esac

# Detect architecture
ARCH=$(uname -m)
case "${ARCH}" in
  x86_64)
    ARCH="x86_64"
    ;;
  arm64|aarch64)
    ARCH="aarch64"
    ;;
  *)
    echo "Error: Unsupported architecture: ${ARCH}"
    exit 1
    ;;
esac

# Map OS+arch to Rust target triple
case "${OS}-${ARCH}" in
  Linux-x86_64)
    TARGET="x86_64-unknown-linux-gnu"
    ;;
  Linux-aarch64)
    TARGET="aarch64-unknown-linux-gnu"
    ;;
  Darwin-x86_64)
    TARGET="x86_64-apple-darwin"
    ;;
  Darwin-aarch64)
    TARGET="aarch64-apple-darwin"
    ;;
  *)
    echo "Error: Unsupported OS-architecture combination: ${OS}-${ARCH}"
    exit 1
    ;;
esac

# Set download URL
URL="https://github.com/dc-powertools/dcc/releases/latest/download/dcc-${TARGET}.tar.gz"

# Print download message
echo "[dcc] Downloading dcc for ${TARGET}..."

# Create temp directory and download
TMPDIR=$(mktemp -d)
curl -fsSL "${URL}" -o "${TMPDIR}/dcc.tar.gz"

# Extract archive
tar -xzf "${TMPDIR}/dcc.tar.gz" -C "${TMPDIR}"

# Create install directory
mkdir -p "${HOME}/.local/bin"

# Move binary to install location
mv "${TMPDIR}/dcc" "${HOME}/.local/bin/dcc"

# Make executable
chmod +x "${HOME}/.local/bin/dcc"

# Clean up temp directory
rm -rf "${TMPDIR}"

# Print success message
echo "[dcc] Installed to ~/.local/bin/dcc"

# Check if ~/.local/bin is on PATH
case ":${PATH}:" in
  *":${HOME}/.local/bin:"*)
    ;;
  *)
    echo "[dcc] Add ~/.local/bin to your PATH to use dcc"
    ;;
esac
