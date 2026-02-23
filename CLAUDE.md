# Beehive — AI Coding Assistant Context

## What is Beehive?

Beehive is a Tauri v2 desktop app for orchestrating coding agents across isolated git workspaces. It lets you manage multiple repos ("hives"), create isolated workspace clones ("combs") on different branches, and run multiple terminal/agent panes side-by-side in each workspace.

**Hierarchy:** Beehive (root dir) → Hives (repos) → Combs (workspace clones) → Terminals (agent panes)

## Tech Stack

- **Frontend:** React 19 + TypeScript 5.8, Vite 7, xterm.js 5.5 (with unicode11 + web-links addons)
- **Backend:** Rust (Tauri v2), portable-pty 0.9, tokio, serde, uuid
- **IPC:** Tauri invoke (commands) + Tauri events (PTY output streaming)
- **Styling:** Plain CSS with CSS custom properties (Catppuccin Mocha theme)
- **Git ops:** `std::process::Command` calling `git` and `gh` CLI directly (no libgit2)

## Directory Structure

```
beehive/
├── src/                    # React frontend
│   ├── App.tsx             # Screen router (loading → preflight → setup → main)
│   ├── App.css             # All styles (Catppuccin Mocha theme)
│   ├── types.ts            # Shared TS types (BeehiveConfig, HiveInfo, Comb, PaneConfig, AppView)
│   ├── main.tsx            # React entry point (StrictMode enabled)
│   └── components/
│       ├── PreflightScreen.tsx   # Checks git/gh/gh-auth availability
│       ├── SetupScreen.tsx       # Directory picker with autocomplete dropdown
│       ├── HiveListScreen.tsx    # Repo CRUD (add via URL, list, delete) — also serves as home/manage view
│       ├── Sidebar.tsx           # Hive selector dropdown + comb list + footer nav
│       ├── MainLayout.tsx        # Orchestrator: sidebar + workspace + overlays, per-hive runtime state
│       ├── WorkspaceGrid.tsx     # Terminal grid for a comb — add/remove panes
│       ├── NewCombModal.tsx      # Modal: comb creation form (name + branch dropdown)
│       ├── SettingsScreen.tsx    # Paths, dependency status, reset
│       └── TerminalPane.tsx      # xterm.js wrapper with PTY IPC, visibility toggling, Unicode 11
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri app builder, registers all commands
│   │   ├── pty.rs          # PTY management (create/write/resize/close), stores child handle
│   │   └── hive.rs         # All hive/comb CRUD, git ops, config, preflight, pane persistence
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

1. **Own PTY management:** The app spawns real PTY sessions via `portable-pty`, not pseudo-terminals. Each pane gets its own PTY with a background reader thread that emits output via Tauri events (`pty-output-{sessionId}`). The frontend writes user input back via `write_to_pty` invoke. The child process handle is stored in `PtySession` to prevent premature termination.

2. **Event-based PTY output:** PTY output is streamed as `Vec<u8>` via Tauri's event system (not command return values) so it can push data asynchronously. The frontend listens with `listen()` from `@tauri-apps/api/event`.

3. **Unique PTY session IDs:** Each `TerminalPane` mount generates a unique session ID (`{paneId}-{uuid}`) for its PTY. This prevents React StrictMode's double-mount/unmount cycle from leaking exit events between PTY instances. The pane ID is stable (persisted), but the session ID is ephemeral per mount.

4. **Git via std::process::Command:** All git and gh operations shell out to the CLI tools directly. No libgit2 binding. This means git and gh must be installed on the user's system (checked at preflight).

5. **camelCase serde:** All Rust structs use `#[serde(rename_all = "camelCase")]` so TypeScript types must use camelCase field names (e.g., `dirName`, `repoUrl`, `defaultBranch`). Keep this consistent when adding new types.

6. **App config at ~/.beehive/config.json:** Stores the beehive directory path. Each beehive directory has a `beehive.json` with version info. Each hive has `.hive/state.json` with repo info, comb list, and pane configs.

7. **Sidebar layout with overlays:** Post-setup, `MainLayout` renders a sidebar + workspace area. Manage Hives and Settings render as `position: fixed` fullscreen overlays on top, keeping the main layout (and all terminals) mounted underneath. This prevents terminal destruction when navigating to settings/hive management.

8. **Per-hive runtime state:** `MainLayout` keeps a `Map<string, HiveRuntime>` where each entry holds `combs`, `openedCombs`, `panesByComb`, and `activeCombId` for a specific hive. Switching hives changes `activeHiveDirName` without destroying any state. ALL opened combs from ALL hives are rendered simultaneously (hidden via `display: none`), keeping PTYs alive across hive switches.

9. **Manage Hives as home screen:** On startup, the Manage Hives overlay is shown first. Users select a hive to enter the sidebar+workspace view. The back button on overlays tracks navigation origin (`from: "sidebar" | "manageHives"`) to return to the correct screen.

10. **PtyState is Arc<Mutex<PtyManager>>:** The PTY manager is shared Tauri state, accessed via `State<'_, PtyState>` in command handlers. Uses tokio::sync::Mutex for async access.

11. **Terminal auto-close on exit:** When a shell process exits (e.g., `exit` command, Ctrl+D), the `onExit` callback removes the pane from the grid automatically.

12. **Pane persistence:** Pane layouts (count, type, position) are saved to disk via `save_comb_panes` / `get_comb_panes` commands. Changes are debounce-saved (500ms). On app restart, pane layout is restored but PTY history is lost (expected).

## State & Config Files

- `~/.beehive/config.json` — App-level config (beehive directory path)
- `{beehiveDir}/beehive.json` — Beehive directory marker with version
- `{beehiveDir}/repo_{name}/.hive/state.json` — Hive state (HiveInfo + combs list + pane configs per comb)
- Combs are full git clones at `{beehiveDir}/repo_{name}/{combName}/`

## Conventions

- Rust error handling: all commands return `Result<T, String>` (error strings for IPC)
- Helper `run_cmd()` in hive.rs wraps `Command::new().output()` with error mapping
- UUID v4 for comb IDs, `crypto.randomUUID()` for pane IDs
- Timestamps are Unix epoch seconds as strings (no chrono crate)
- CSS uses Catppuccin Mocha color tokens via custom properties
- Font stack prioritizes Nerd Fonts for full Unicode/powerline symbol support
- Delete confirmations use a simple two-click pattern (X → "Sure?" → delete), no countdown timers

## Navigation Flow

```
App startup → Preflight → Setup (if first run) → MainLayout
                                                    ├── Manage Hives (home/overlay)
                                                    │   ├── Select hive → sidebar+workspace
                                                    │   └── Settings (overlay, back → Manage Hives)
                                                    ├── Sidebar
                                                    │   ├── Hive dropdown (switch between hives)
                                                    │   ├── Comb list (click to open, x to delete)
                                                    │   ├── + New Comb (modal)
                                                    │   ├── Manage Hives (overlay)
                                                    │   └── Settings (overlay, back → workspace)
                                                    └── Workspace (terminal grid per comb)
```
