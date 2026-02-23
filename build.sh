#!/usr/bin/env bash
set -euo pipefail

# Beehive build script (macOS)
# Produces a .app bundle and .dmg installer.
#
# Usage:
#   ./build.sh          Build production app
#   ./build.sh --dev    Run in development mode
#   ./build.sh --check  Type-check only (no build)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# ── Helpers ──────────────────────────────────────────────────────────────────

red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
dim()   { printf '\033[0;90m%s\033[0m\n' "$*"; }

check_cmd() {
  if ! command -v "$1" &>/dev/null; then
    red "Error: $1 is not installed."
    echo "$2"
    exit 1
  fi
}

# ── Preflight checks ────────────────────────────────────────────────────────

echo "Checking prerequisites..."

check_cmd node   "Install Node.js 18+: https://nodejs.org/"
check_cmd npm    "Install Node.js 18+: https://nodejs.org/"
check_cmd rustc  "Install Rust: https://rustup.rs/"
check_cmd cargo  "Install Rust: https://rustup.rs/"
check_cmd git    "Install git: https://git-scm.com/"

# Ensure cargo is on PATH (common issue on macOS)
if [ -f "$HOME/.cargo/env" ]; then
  source "$HOME/.cargo/env"
fi

NODE_VER=$(node --version)
RUST_VER=$(rustc --version)
dim "  Node: $NODE_VER"
dim "  Rust: $RUST_VER"
echo ""

# ── Install dependencies ────────────────────────────────────────────────────

if [ ! -d "node_modules" ]; then
  echo "Installing npm dependencies..."
  npm install
  echo ""
fi

# ── Parse arguments ─────────────────────────────────────────────────────────

MODE="build"
if [ "${1:-}" = "--dev" ]; then
  MODE="dev"
elif [ "${1:-}" = "--check" ]; then
  MODE="check"
fi

# ── Execute ─────────────────────────────────────────────────────────────────

case "$MODE" in
  check)
    echo "Running type checks..."
    echo ""
    echo "TypeScript:"
    npx tsc --noEmit
    green "  TypeScript OK"
    echo ""
    echo "Rust:"
    (cd src-tauri && cargo check)
    green "  Rust OK"
    echo ""
    green "All checks passed."
    ;;

  dev)
    echo "Starting development server..."
    npm run tauri dev
    ;;

  build)
    echo "Building production app..."
    echo ""
    npm run tauri build
    echo ""

    # Find output
    if [ -d "src-tauri/target/release/bundle/macos" ]; then
      APP_PATH=$(find src-tauri/target/release/bundle/macos -name "*.app" -maxdepth 1 | head -1)
      DMG_PATH=$(find src-tauri/target/release/bundle/dmg -name "*.dmg" 2>/dev/null | head -1)
      echo ""
      green "Build complete!"
      echo ""
      echo "Outputs:"
      [ -n "${APP_PATH:-}" ] && echo "  App: $APP_PATH"
      [ -n "${DMG_PATH:-}" ] && echo "  DMG: $DMG_PATH"
      echo ""
      echo "Install:"
      echo "  cp -r \"$APP_PATH\" /Applications/"
    elif [ -d "src-tauri/target/release/bundle" ]; then
      echo ""
      green "Build complete!"
      echo "  Output: src-tauri/target/release/bundle/"
    fi
    ;;
esac
