# Beehive — Project Architecture

## Overview

Beehive is a Tauri v2 desktop application for orchestrating coding agents across isolated git workspaces. It provides a visual interface for managing repositories, creating branch-specific workspace clones, and running multiple terminal sessions (for agents or manual work) in a grid layout.

## System Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Tauri Window                       │
│  ┌───────────────────────────────────────────────┐  │
│  │              React Frontend                    │  │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────────┐  │  │
│  │  │ Screen  │ │ Screen   │ │  Workspace    │  │  │
│  │  │ Router  │ │Components│ │  Terminal Grid │  │  │
│  │  │(App.tsx)│ │          │ │  (xterm.js)   │  │  │
│  │  └────┬────┘ └────┬─────┘ └───────┬───────┘  │  │
│  │       │           │               │           │  │
│  │       ▼           ▼               ▼           │  │
│  │  ┌─────────────────────────────────────────┐  │  │
│  │  │         Tauri IPC Layer                 │  │  │
│  │  │   invoke() ──── commands (req/res)      │  │  │
│  │  │   listen() ──── events (push)           │  │  │
│  │  └─────────────────┬───────────────────────┘  │  │
│  └────────────────────│──────────────────────────┘  │
│                       │                              │
│  ┌────────────────────▼──────────────────────────┐  │
│  │              Rust Backend                      │  │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐  │  │
│  │  │  pty.rs  │  │ hive.rs  │  │  lib.rs    │  │  │
│  │  │ PTY mgmt │  │ Git ops  │  │ App entry  │  │  │
│  │  │ sessions │  │ File I/O │  │ Cmd registry│  │  │
│  │  └────┬─────┘  └────┬─────┘  └────────────┘  │  │
│  │       │              │                         │  │
│  │       ▼              ▼                         │  │
│  │  portable-pty    std::process::Command         │  │
│  │  (real PTYs)     (git, gh CLI)                 │  │
│  └────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Data Model

### Hierarchy

```
~/.beehive/config.json          ← app-level config (points to beehive dir)
    │
    ▼
{beehiveDir}/
├── beehive.json                ← beehive marker file (version)
├── repo_myapp/                 ← hive directory
│   ├── .hive/
│   │   └── state.json          ← HiveState { info: HiveInfo, combs: Comb[] }
│   ├── feature-auth/           ← comb (full git clone, checked out to branch)
│   │   └── ... (repo files)
│   └── bugfix-login/           ← another comb
│       └── ... (repo files)
└── repo_otherproject/          ← another hive
    └── .hive/
        └── state.json
```

### Key Types (Rust → TypeScript via serde camelCase)

| Rust Struct | TS Interface | Purpose |
|-------------|-------------|---------|
| `AppConfig` | `AppConfig` | `{ beehiveDir: string \| null }` |
| `BeehiveConfig` | `BeehiveConfig` | `{ version: number, beehiveDir: string }` |
| `HiveInfo` | `HiveInfo` | Repo metadata: dirName, repoUrl, owner, repoName, description, defaultBranch |
| `Comb` | `Comb` | Workspace: id (uuid), name, branch, path, createdAt |
| `HiveState` | `HiveState` | `{ info: HiveInfo, combs: Comb[] }` |
| `PreflightResult` | `PreflightResult` | Dependency check results |
| `RepoBranch` | `RepoBranch` | `{ name: string, isDefault: boolean }` |
| `DirEntry` | `DirEntry` | Directory listing entry for setup autocomplete |

## Screen Flow

```
┌──────────┐     ┌───────────┐     ┌───────────┐     ┌────────────┐     ┌─────────────┐
│ Loading  │────▶│ Preflight │────▶│   Setup   │────▶│ Hive List  │────▶│  Comb List  │
│          │     │  Screen   │     │  Screen   │     │   Screen   │     │   Screen    │
└──────────┘     └─────┬─────┘     └───────────┘     └──────┬─────┘     └──────┬──────┘
                       │                                     │                  │
                       │ (if beehiveDir exists,              │                  ▼
                       │  skip setup)                        │           ┌─────────────┐
                       └─────────────────────────────────────┘           │  Workspace  │
                                                                         │   Screen    │
                       ┌──────────────┐                                  │ (terminals) │
                       │   Settings   │◀── gear icon from Hive List     └─────────────┘
                       │   Screen     │
                       └──────────────┘
```

### Screen Descriptions

1. **Loading** — Brief splash while `load_app_config` runs.
2. **Preflight** — Checks git, gh CLI, and gh auth status. Auto-advances on success (800ms delay). Shows retry button on failure.
3. **Setup** — Directory picker for the beehive root. Text input with debounced filesystem autocomplete (150ms), arrow key navigation, tab completion, and a native browse dialog fallback. Creates `beehive.json` on confirm.
4. **Hive List** — Shows all linked repos. Add form accepts owner/repo, HTTPS URL, or SSH URL. Verification calls `gh repo view` then `git ls-remote`. Auto-cleans broken hive directories.
5. **Comb List** — Shows workspaces for a hive. Create form has name input and a custom-built branch dropdown with search filter. Creating a comb runs `git clone` + `git checkout`.
6. **Workspace** — Terminal grid. Start with one terminal pane. Add more terminal or agent panes. Responsive columns (1/2/3). Each pane is an independent PTY session.
7. **Settings** — Shows paths, dependency status, and reset button (with double-confirm).

## PTY Data Flow

### Creating a Terminal

```
Frontend                              Backend (pty.rs)
   │                                      │
   │ invoke("create_pty", {id, cwd,       │
   │   cmd, args, rows, cols})            │
   │─────────────────────────────────────▶│
   │                                      │ openpty(PtySize)
   │                                      │ spawn_command(shell or cmd)
   │                                      │ take_writer() → store in PtySession
   │                                      │ try_clone_reader() → spawn reader thread
   │                                      │ Store session in PtyManager.sessions
   │◀─────────────────────────────────────│ Ok(())
   │                                      │
   │        Background reader thread:     │
   │        loop { read(&mut buf) }       │
   │                                      │
   │ listen("pty-output-{id}")            │
   │◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │ emit("pty-output-{id}", Vec<u8>)
   │                                      │
   │ terminal.write(Uint8Array)           │
   │                                      │
```

### User Input

```
Frontend                              Backend (pty.rs)
   │                                      │
   │ terminal.onData(data)                │
   │ invoke("write_to_pty", {id, data})   │
   │─────────────────────────────────────▶│ writer.write_all(data.as_bytes())
   │                                      │ writer.flush()
   │                                      │
```

### Resize

```
Frontend                              Backend (pty.rs)
   │                                      │
   │ ResizeObserver fires                 │
   │ fitAddon.fit()                       │
   │ invoke("resize_pty", {id, rows, cols})│
   │─────────────────────────────────────▶│ master.resize(PtySize)
   │                                      │
```

## Hive/Comb Operations

### Adding a Hive

```
Frontend                              Backend (hive.rs)
   │                                      │
   │ invoke("create_hive",               │
   │   {beehiveDir, repoUrl})            │
   │─────────────────────────────────────▶│
   │                                      │ parse_repo_url(url)
   │                                      │   → supports: owner/repo, HTTPS, SSH
   │                                      │ gh repo view owner/repo --json ...
   │                                      │   → get name, description, defaultBranch, URLs
   │                                      │ git ls-remote --heads <clone_url>
   │                                      │   → verify repo is cloneable
   │                                      │ mkdir repo_{name}/.hive/
   │                                      │ write state.json
   │◀─────────────────────────────────────│ Ok(HiveInfo)
```

### Creating a Comb

```
Frontend                              Backend (hive.rs)
   │                                      │
   │ invoke("create_comb",               │
   │   {beehiveDir, dirName,             │
   │    name, branch})                    │
   │─────────────────────────────────────▶│
   │                                      │ load .hive/state.json
   │                                      │ git clone <repoUrl> <combDir>
   │                                      │ git checkout <branch>
   │                                      │   (or git checkout -b <branch>)
   │                                      │ Generate UUID for comb ID
   │                                      │ Append to combs[], save state.json
   │◀─────────────────────────────────────│ Ok(Comb)
```

## State Management

The app uses React's `useState` for all state — no external state library.

- **App.tsx** holds the top-level `screen` state and `beehiveDir`. All navigation is via callback props passed down to screen components.
- **Each screen** manages its own local state (loading flags, form values, lists fetched from backend).
- **No global store.** Each screen fetches what it needs via `invoke()` on mount.
- **Backend is the source of truth.** The filesystem (state.json files) is the persistent store. The frontend re-fetches on navigation.

## File-by-File Descriptions

### Backend (src-tauri/src/)

**lib.rs** — Tauri application entry point. Creates the `PtyManager` as shared state (`Arc<Mutex<PtyManager>>`), registers all Tauri plugins (opener, dialog), and registers all command handlers from `pty.rs` and `hive.rs`.

**pty.rs** — PTY session management. Defines `PtySession` (holds master + writer Arc<Mutex>), `PtyManager` (HashMap of sessions), and four commands:
- `create_pty` — Opens a PTY, spawns a shell or custom command, starts a background reader thread that emits output events.
- `write_to_pty` — Writes string data to a PTY's stdin.
- `resize_pty` — Resizes a PTY (updates terminal dimensions).
- `close_pty` — Removes a session from the manager (drops PTY).

**hive.rs** — Everything else. Contains all data structs with `#[serde(rename_all = "camelCase")]`, helper functions (`run_cmd`, `parse_repo_url`, `load_hive_state`, `save_hive_state`), and commands for:
- Preflight checks (`preflight_check`)
- App config CRUD (`load_app_config`, `save_app_config`, `reset_app`, `get_app_config_path`)
- Directory operations (`get_home_dir`, `list_dirs`)
- Beehive init/load (`init_beehive`, `load_beehive`)
- Hive CRUD (`verify_repo`, `create_hive`, `list_hives`, `delete_hive`)
- Branch listing (`list_branches` — via gh API)
- Comb CRUD (`create_comb`, `list_combs`, `delete_comb`)

### Frontend (src/)

**App.tsx** — Screen router. Holds `screen` state (discriminated union) and `beehiveDir`. Renders one screen component at a time. Handles setup flow: load config → preflight → setup (if needed) → hive list.

**types.ts** — Shared TypeScript interfaces matching Rust structs (camelCase). Defines `BeehiveConfig`, `HiveInfo`, `Comb`, `HiveState`, `PaneInfo`, and `AppView` discriminated union.

**App.css** — All application styles. Uses CSS custom properties for theming (Catppuccin Mocha palette). Defines styles for screens, cards, forms, lists, workspace grid, terminal panes, custom dropdown, autocomplete, and utility classes.

### Frontend Components (src/components/)

**PreflightScreen.tsx** — Runs `preflight_check` on mount. Shows check items (git, gh, gh auth) with status indicators. Auto-advances after 800ms if all pass. Shows error messages and retry button on failure.

**SetupScreen.tsx** — Directory picker for beehive root. Features: pre-fills with `~/beehive`, debounced autocomplete (150ms) via `list_dirs`, keyboard navigation (arrows, tab, enter, escape), native browse dialog via `@tauri-apps/plugin-dialog`, calls `init_beehive` on confirm.

**HiveListScreen.tsx** — Lists hives from `list_hives`. Add form with repo URL input (autocomplete/spellcheck disabled). Client-side format validation before invoking `create_hive`. Delete with confirmation dialog. Settings button navigates to SettingsScreen.

**CombListScreen.tsx** — Lists combs for a hive. Custom-built branch dropdown with search filter (no native select). Fetches branches via `list_branches` (gh API). Creates combs via `create_comb` (git clone + checkout). Handles dropdown open/close, keyboard events, outside-click dismissal.

**WorkspaceScreen.tsx** — Terminal grid layout. Starts with one terminal pane. Add buttons for "Terminal" (default shell) and "Agent" (runs `claude` command). Panes arranged in responsive grid (1 col for 1 pane, 2 cols for 2-4, 3 cols for 5+). Each pane has a header badge (TERM/AGENT) and close button.

**TerminalPane.tsx** — xterm.js wrapper. On mount: creates Terminal instance with Catppuccin theme, loads FitAddon + WebLinksAddon, fits after `requestAnimationFrame`, then creates PTY via invoke. Listens for `pty-output-{id}` events to write to terminal. Sends user input via `write_to_pty`. Handles resize via ResizeObserver + `resize_pty`. Cleanup on unmount: disconnect observer, unlisten events, close PTY, dispose terminal.

**SettingsScreen.tsx** — Shows beehive dir path, config file path, dependency status (re-runs preflight). Reset button with double-confirmation pattern (click once to arm, click again to confirm). Reset calls `reset_app` which deletes `~/.beehive/config.json`.

## Future Plans

See `plan.md` for the full roadmap. Key upcoming items:

- **Short term:** Verification step when adding hives, progress indicators, end-to-end testing of clone + terminal workflows, keyboard shortcuts, resizable panes
- **Medium term:** Environment management, agent configuration per hive, git sync (pull/push), git status sidebar, multiple agent support, persistent pane layouts
- **Long term:** Onboarding wizard, vim-style navigation, comb templates, shared secrets, activity log, resource monitoring dashboard
