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
  Linux) OS_LABEL="linux" ;;
  *)
    echo "Error: Unsupported OS: $OS (supported: macOS Apple Silicon, Linux x64)"
    exit 1
    ;;
esac

if [ "$OS_LABEL" = "darwin" ]; then
  case "$ARCH" in
    arm64|aarch64) ARCH_LABEL="arm64" ;;
    *)
      echo "Error: Unsupported architecture: $ARCH (macOS builds currently support Apple Silicon only)"
      exit 1
      ;;
  esac
else
  case "$ARCH" in
    x86_64|amd64) ARCH_LABEL="x64" ;;
    *)
      echo "Error: Unsupported architecture: $ARCH (Linux builds currently support x64 only)"
      exit 1
      ;;
  esac
fi

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

# Choose command name
CMD_NAME="${BH_CMD_NAME:-}"
if [ -z "$CMD_NAME" ]; then
  if [ -t 0 ]; then
    echo ""
    echo "Command name?"
    echo "  [1] bh       (short, recommended)"
    echo "  [2] beehive  (full name)"
    printf "Choice [1]: "
    read -r choice
    case "$choice" in
      2|beehive) CMD_NAME="beehive" ;;
      *) CMD_NAME="bh" ;;
    esac
  else
    CMD_NAME="bh"
  fi
fi

echo "Installing to ${INSTALL_DIR}/${CMD_NAME}..."
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPFILE" "${INSTALL_DIR}/${CMD_NAME}"
else
  sudo mv "$TMPFILE" "${INSTALL_DIR}/${CMD_NAME}"
fi

echo "Installed ${CMD_NAME} ${LATEST_TAG} to ${INSTALL_DIR}/${CMD_NAME}"
echo "Run '${CMD_NAME}' to start."
