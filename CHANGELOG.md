# Changelog

## 0.1.3 — 2026-02-23

### Bug Fixes
- Fix file/image drag-and-drop into terminal panes. Tauri v2 intercepts native drag events by default, preventing files from reaching the terminal. Now listens for Tauri's drag-drop events and pastes file paths through xterm.js with proper bracketed paste handling, so coding agents (OpenCode, Claude Code) correctly detect dropped images.
- Added drag-over visual feedback (dashed blue border overlay) when hovering a file over a terminal pane.

## 0.1.2 — 2026-02-22 — Initial Release

### Core Infrastructure
- Tauri v2 scaffold with React + TypeScript + Vite frontend
- Rust backend with portable-pty for real pseudo-terminal sessions
- Tauri event-based PTY output streaming (`pty-output-{id}`, `pty-exit-{id}`)
- PTY commands: create, write, resize, close

### Preflight & Setup
- Preflight screen checks for git, gh CLI, and gh authentication
- Setup screen with directory picker (text input with filesystem autocomplete + native browse dialog)
- App config persisted at `~/.beehive/config.json`

### Hive Management (Repos)
- Add hives by GitHub URL (HTTPS, SSH, or owner/repo shorthand)
- Repo verification via `gh repo view` and `git ls-remote`
- List hives with owner, name, and description
- Delete hives with full directory cleanup
- Auto-cleanup of broken/orphaned hive directories on list

### Comb Management (Workspaces)
- Create combs as full git clones of a hive repo
- Custom branch selector dropdown with search/filter
- Branch checkout (existing branch or create new)
- Delete combs with workspace directory cleanup
- Comb state persisted in `.hive/state.json`

### Workspace & Terminals
- Terminal grid with dynamic pane add/remove
- Two pane types: "terminal" (user shell) and "agent" (runs `claude` command)
- xterm.js with Catppuccin Mocha theme, FitAddon, WebLinksAddon
- Automatic PTY resize on pane resize via ResizeObserver
- Responsive grid layout (1/2/3 columns based on pane count)

### Settings
- View beehive directory path and config file location
- Dependency status display (git, gh, gh auth)
- Reset functionality with confirmation (clears config, preserves files on disk)

### UI/UX
- Dark theme (Catppuccin Mocha) with CSS custom properties
- Custom-built components (no UI library): dropdown select, autocomplete, form inputs
- Input sanitization: autocomplete/spellcheck disabled on repo URL input
- Client-side URL format validation before network calls

### Bug Fixes
- Fixed repo URL parsing validation for empty owner/repo segments
- Fixed hive deletion not persisting (delete now removes directory)
- Disabled browser autocomplete/spellcheck on repo input field
- Added auto-cleanup of broken hive directories on hive list load
