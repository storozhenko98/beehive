#!/bin/bash
set -euo pipefail

REPO="storozhenko98/beehive"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="beehive-tui"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) OS_LABEL="darwin" ;;
  *)
    echo "Error: Unsupported OS: $OS (only macOS is supported currently)"
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH_LABEL="arm64" ;;
  *)
    echo "Error: Unsupported architecture: $ARCH (only Apple Silicon is supported currently)"
    exit 1
    ;;
esac

ASSET_NAME="${BINARY_NAME}-${OS_LABEL}-${ARCH_LABEL}"

# Get latest release tag
echo "Fetching latest release..."
LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
  echo "Error: Could not determine latest release"
  exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${ASSET_NAME}"

echo "Downloading ${BINARY_NAME} ${LATEST_TAG}..."
TMPFILE=$(mktemp)
curl -fsSL -o "$TMPFILE" "$DOWNLOAD_URL"
chmod +x "$TMPFILE"

echo "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPFILE" "${INSTALL_DIR}/${BINARY_NAME}"
else
  sudo mv "$TMPFILE" "${INSTALL_DIR}/${BINARY_NAME}"
fi

echo "Installed ${BINARY_NAME} ${LATEST_TAG} to ${INSTALL_DIR}/${BINARY_NAME}"
echo "Run 'beehive-tui' to start."
