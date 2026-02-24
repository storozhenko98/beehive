#!/usr/bin/env bash
set -euo pipefail

# Beehive build script (macOS)
# Produces a signed .app bundle and .dmg installer.
#
# Usage:
#   ./build.sh            Build production app (signed + notarized)
#   ./build.sh --dev      Run in development mode
#   ./build.sh --check    Type-check only (no build)
#   ./build.sh --release  Build, notarize, and publish to GitHub Releases

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Source .env if it exists (for local builds)
if [ -f ".env" ]; then
  set -a
  source .env
  set +a
fi

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

# ── Config ───────────────────────────────────────────────────────────────────

NOTARIZE_PROFILE="beehive-notarize"
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
REPO="storozhenko98/beehive"

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
dim "  Version: $VERSION"
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
elif [ "${1:-}" = "--release" ]; then
  MODE="release"
fi

# ── Functions ────────────────────────────────────────────────────────────────

do_build() {
  echo "Building production app..."
  echo ""

  # Set notarization env vars — Tauri reads these automatically
  for var in APPLE_ID APPLE_TEAM_ID APPLE_PASSWORD TAURI_SIGNING_PRIVATE_KEY; do
    if [ -z "${!var:-}" ]; then
      red "Error: $var not set."
      echo "Set notarization env vars before running:"
      echo "  export APPLE_ID='your@email.com'"
      echo "  export APPLE_TEAM_ID='XXXXXXXXXX'"
      echo "  export APPLE_PASSWORD='xxxx-xxxx-xxxx-xxxx'"
      echo "  export TAURI_SIGNING_PRIVATE_KEY='...'"
      exit 1
    fi
  done

  npm run tauri build
  echo ""
}

find_artifacts() {
  APP_PATH=""
  DMG_PATH=""
  if [ -d "src-tauri/target/release/bundle/macos" ]; then
    APP_PATH=$(find src-tauri/target/release/bundle/macos -name "*.app" -maxdepth 1 | head -1)
  fi
  if [ -d "src-tauri/target/release/bundle/dmg" ]; then
    DMG_PATH=$(find src-tauri/target/release/bundle/dmg -name "*.dmg" 2>/dev/null | head -1)
  fi
}

print_artifacts() {
  find_artifacts
  echo "Outputs:"
  [ -n "${APP_PATH:-}" ] && echo "  App: $APP_PATH"
  [ -n "${DMG_PATH:-}" ] && echo "  DMG: $DMG_PATH"
  echo ""
  echo "Install:"
  [ -n "${APP_PATH:-}" ] && echo "  cp -r \"$APP_PATH\" /Applications/"
}

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
    do_build
    green "Build complete! (signed + notarized)"
    echo ""
    print_artifacts
    ;;

  release)
    check_cmd gh "Install GitHub CLI: https://cli.github.com/"

    TAG="v${VERSION}"
    echo "Building release ${TAG}..."
    echo ""

    do_build

    find_artifacts

    if [ -z "${DMG_PATH:-}" ]; then
      red "Error: No .dmg found after build."
      exit 1
    fi

    green "Build complete! (signed + notarized)"
    echo ""
    print_artifacts
    echo ""

    # Check if release already exists
    if gh release view "$TAG" --repo "$REPO" &>/dev/null; then
      echo "Release $TAG already exists. Uploading artifacts..."
      gh release upload "$TAG" "$DMG_PATH" --repo "$REPO" --clobber
    else
      echo "Creating GitHub release $TAG..."
      gh release create "$TAG" \
        "$DMG_PATH" \
        --repo "$REPO" \
        --title "Beehive $TAG" \
        --generate-notes
    fi

    green "Release $TAG published!"
    echo "  https://github.com/$REPO/releases/tag/$TAG"
    ;;
esac
