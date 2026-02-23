# Contributing to Beehive

Thanks for your interest in contributing! This document covers everything you need to get started.

## Development Setup

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| **Node.js** | 18+ | [nodejs.org](https://nodejs.org/) |
| **Rust** | stable (latest) | [rustup.rs](https://rustup.rs/) |
| **git** | any recent | [git-scm.com](https://git-scm.com/) |
| **GitHub CLI** | any recent | [cli.github.com](https://cli.github.com/) |

The GitHub CLI must be authenticated: `gh auth login`

### macOS

- Xcode Command Line Tools: `xcode-select --install`

> **Linux & Windows:** Beehive currently only supports macOS. Tauri v2 supports all three platforms, so porting should be feasible. If you'd like to help, see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for platform-specific requirements. PRs for Linux/Windows support are very welcome.

### Getting started

```bash
git clone https://github.com/YOUR_USERNAME/beehive.git
cd beehive
npm install
npm run tauri dev
```

This starts the Vite dev server and the Tauri app together with hot-reload.

### Useful commands

```bash
# Type-check everything (no build)
./build.sh --check

# Development mode with hot-reload
./build.sh --dev

# Production build
./build.sh

# Frontend type-check only
npx tsc --noEmit

# Rust type-check only
source "$HOME/.cargo/env" && cd src-tauri && cargo check
```

## Project Structure

```
beehive/
├── src/                        # React frontend (TypeScript)
│   ├── App.tsx                 # Screen router
│   ├── App.css                 # All styles (Catppuccin Mocha theme)
│   ├── types.ts                # Shared types
│   └── components/
│       ├── MainLayout.tsx      # Sidebar + workspace orchestrator
│       ├── Sidebar.tsx         # Hive/comb navigation
│       ├── WorkspaceGrid.tsx   # Terminal pane grid
│       ├── TerminalPane.tsx    # xterm.js + PTY integration
│       ├── HiveListScreen.tsx  # Repo management
│       ├── NewCombModal.tsx    # Comb creation
│       ├── SettingsScreen.tsx  # App settings
│       ├── HelpScreen.tsx      # In-app help
│       ├── PreflightScreen.tsx # Dependency checker
│       └── SetupScreen.tsx     # First-run setup
├── src-tauri/
│   └── src/
│       ├── lib.rs              # Tauri app builder, command registration
│       ├── pty.rs              # PTY session management
│       └── hive.rs             # Hive/comb CRUD, git ops, config
├── public/                     # Static assets
├── build.sh                    # Build script
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## Architecture Notes

- **Frontend ↔ Backend:** Communication uses Tauri's `invoke()` for commands and `listen()` for event streams (PTY output).
- **PTY lifecycle:** Each terminal pane spawns a real PTY via `portable-pty`. A background thread reads output and emits Tauri events. The child process handle is stored to prevent premature termination.
- **State persistence:** Pane layouts are saved to disk (debounced). Terminal history is not persisted across restarts.
- **Sidebar layout:** All opened combs from all hives are rendered simultaneously. Inactive combs are hidden via `display: none` to keep their terminals alive.
- **Serde convention:** All Rust structs use `#[serde(rename_all = "camelCase")]`. TypeScript types must match.

## Making Changes

1. **Fork and branch** — Create a feature branch from `master`.
2. **Type-check** — Run `./build.sh --check` before committing. Both TypeScript and Rust must pass.
3. **Keep it focused** — One feature or fix per PR. Small PRs get reviewed faster.
4. **Test manually** — Run `npm run tauri dev` and verify your changes work end-to-end.
5. **Write clear commits** — Describe what changed and why.

## Code Style

- **TypeScript:** Strict mode enabled. No unused locals or parameters.
- **Rust:** Standard rustfmt. All commands return `Result<T, String>`.
- **CSS:** Plain CSS with custom properties. Follow existing Catppuccin Mocha theme tokens.
- **No linter/formatter configs** — just follow existing patterns in the codebase.

## Reporting Issues

When filing a bug, please include:
- Your OS and version
- Steps to reproduce
- What you expected vs what happened
- Console output if relevant (`View → Toggle Developer Tools` in the app)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
