#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCKERFILE="${ROOT_DIR}/docker/tui-linux/Dockerfile"
IMAGE_NAME="${BEEHIVE_TUI_LINUX_IMAGE:-beehive-tui-linux-test}"
PLATFORM="${BEEHIVE_TUI_LINUX_PLATFORM:-linux/amd64}"
ARTIFACT_DIR="${ROOT_DIR}/dist/linux"
ARTIFACT_PATH="${ARTIFACT_DIR}/beehive-tui-linux-x64"

usage() {
  cat <<'EOF'
Usage: ./scripts/tui-linux-docker.sh <command>

Commands:
  build-image  Build the Linux test image.
  check        Run cargo check for the TUI inside Docker.
  build        Run a release build for the TUI inside Docker.
  export       Build a release binary and copy it to dist/linux/beehive-tui-linux-x64.
  run          Build and launch the TUI interactively inside Docker.
  shell        Open an interactive shell inside the Linux test container.
EOF
}

build_image() {
  docker build \
    --platform "${PLATFORM}" \
    --tag "${IMAGE_NAME}" \
    --file "${DOCKERFILE}" \
    "${ROOT_DIR}/docker/tui-linux"
}

run_in_container() {
  docker run --rm \
    --platform "${PLATFORM}" \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace/cli \
    "${IMAGE_NAME}" \
    "$@"
}

run_interactive() {
  docker run --rm -it \
    --platform "${PLATFORM}" \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace/cli \
    "${IMAGE_NAME}" \
    "$@"
}

command_name="${1:-}"

case "${command_name}" in
  build-image)
    build_image
    ;;
  check)
    build_image
    run_in_container cargo check
    ;;
  build)
    build_image
    run_in_container cargo build --release
    ;;
  export)
    build_image
    mkdir -p "${ARTIFACT_DIR}"
    run_in_container /bin/bash -c \
      "cargo build --release && install -m 755 target/release/beehive-tui /workspace/dist/linux/beehive-tui-linux-x64"
    echo "Exported Linux binary to ${ARTIFACT_PATH}"
    ;;
  run)
    build_image
    run_interactive /bin/bash -c "cargo build --release && exec target/release/beehive-tui"
    ;;
  shell)
    build_image
    run_interactive /bin/bash
    ;;
  *)
    usage
    exit 1
    ;;
esac
