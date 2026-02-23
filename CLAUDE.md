# Beehive — AI Coding Assistant Context

## What is Beehive?

Beehive is a Tauri v2 desktop app for orchestrating coding agents across isolated git workspaces. It lets you manage multiple repos ("hives"), create isolated workspace clones ("combs") on different branches, and run multiple terminal/agent panes side-by-side in each workspace.

**Hierarchy:** Beehive (root dir) → Hives (repos) → Combs (workspace clones) → Terminals (agent panes)

## Tech Stack

- **Frontend:** React 19 + TypeScript 5.8, Vite 7, xterm.js 5.5
- **Backend:** Rust (Tauri v2), portable-pty 0.9, tokio, serde, uuid
- **IPC:** Tauri invoke (commands) + Tauri events (PTY output streaming)
- **Styling:** Plain CSS with CSS custom properties (Catppuccin Mocha theme)
- **Git ops:** `std::process::Command` calling `git` and `gh` CLI directly (no libgit2)

## Directory Structure

```
beehive/
├── src/                    # React frontend
│   ├── App.tsx             # Screen router (loading → preflight → setup → hives → combs → workspace)
│   ├── App.css             # All styles (Catppuccin Mocha theme)
│   ├── types.ts            # Shared TS types (BeehiveConfig, HiveInfo, Comb, PaneInfo, AppView)
│   ├── main.tsx            # React entry point
│   └── components/
│       ├── PreflightScreen.tsx   # Checks git/gh/gh-auth availability
│       ├── SetupScreen.tsx       # Directory picker with autocomplete dropdown
│       ├── HiveListScreen.tsx    # Repo CRUD (add via URL, list, delete)
│       ├── CombListScreen.tsx    # Workspace CRUD with custom branch dropdown
│       ├── WorkspaceScreen.tsx   # Terminal grid — add/remove panes
│       ├── SettingsScreen.tsx    # Paths, dependency status, reset
│       └── TerminalPane.tsx      # xterm.js wrapper with PTY IPC
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri app builder, registers all commands
│   │   ├── pty.rs          # PTY management (create/write/resize/close)
│   │   └── hive.rs         # All hive/comb CRUD, git ops, config, preflight
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri config (window 1400x900, dev port 1420)
├── plan.md                 # TODO list and design notes
├── package.json            # Node dependencies
└── vite.config.ts          # Vite config
```

## How to Run

```bash
# Install frontend dependencies
cd /Users/nikita/beehive && npm install

# Run in development mode (starts both Vite dev server and Tauri)
npm run tauri dev
```

## Common Commands

```bash
# Rust type-check (from src-tauri/)
source "$HOME/.cargo/env" && cargo check

# TypeScript type-check
cd /Users/nikita/beehive && npx tsc --noEmit

# Full dev build
cd /Users/nikita/beehive && npm run tauri dev

# Production build
cd /Users/nikita/beehive && npm run tauri build
```

**Important:** Always run `source "$HOME/.cargo/env"` before any `cargo` commands to ensure the Rust toolchain is on PATH.

## Key Architecture Decisions

1. **Own PTY management:** The app spawns real PTY sessions via `portable-pty`, not pseudo-terminals. Each pane gets its own PTY with a background reader thread that emits output via Tauri events (`pty-output-{id}`). The frontend writes user input back via `write_to_pty` invoke.

2. **Event-based PTY output:** PTY output is streamed as `Vec<u8>` via Tauri's event system (not command return values) so it can push data asynchronously. The frontend listens with `listen()` from `@tauri-apps/api/event`.

3. **Git via std::process::Command:** All git and gh operations shell out to the CLI tools directly. No libgit2 binding. This means git and gh must be installed on the user's system (checked at preflight).

4. **camelCase serde:** All Rust structs use `#[serde(rename_all = "camelCase")]` so TypeScript types must use camelCase field names (e.g., `dirName`, `repoUrl`, `defaultBranch`). Keep this consistent when adding new types.

5. **App config at ~/.beehive/config.json:** Stores the beehive directory path. Each beehive directory has a `beehive.json` with version info. Each hive has `.hive/state.json` with repo info and comb list.

6. **Screen-based routing:** No router library. `App.tsx` holds a `screen` state that determines which component renders. Navigation is via callback props.

7. **PtyState is Arc<Mutex<PtyManager>>:** The PTY manager is shared Tauri state, accessed via `State<'_, PtyState>` in command handlers. Uses tokio::sync::Mutex for async access.

## State & Config Files

- `~/.beehive/config.json` — App-level config (beehive directory path)
- `{beehiveDir}/beehive.json` — Beehive directory marker with version
- `{beehiveDir}/repo_{name}/.hive/state.json` — Hive state (HiveInfo + combs list)
- Combs are full git clones at `{beehiveDir}/repo_{name}/{combName}/`

## Conventions

- Rust error handling: all commands return `Result<T, String>` (error strings for IPC)
- Helper `run_cmd()` in hive.rs wraps `Command::new().output()` with error mapping
- UUID v4 for comb IDs
- Timestamps are Unix epoch seconds as strings (no chrono crate)
- CSS uses Catppuccin Mocha color tokens via custom properties
