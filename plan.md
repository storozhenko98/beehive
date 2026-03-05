# Beehive — Plan & Notes

## Architecture
Beehive (root dir) -> Hives (repos) -> Combs (workspaces/branches) -> Terminals (agent panes)

Tauri v2 + React + TypeScript frontend, Rust backend with portable-pty + xterm.js.

## Current State
- [x] Tauri scaffold + build pipeline
- [x] PTY backend (create/write/resize/close)
- [x] Preflight checks (git, gh, gh auth)
- [x] Setup screen with dir picker (browse + autocomplete)
- [x] Hive list (CRUD via gh)
- [x] Comb list (git clone + branch checkout)
- [x] Workspace screen with terminal grid
- [x] Fix: repo URL parsing validation (empty owner/repo)
- [x] Fix: delete hive persistence
- [x] Fix: autocomplete/spellcheck disabled on repo input
- [x] Fix: auto-cleanup broken hive dirs on list
- [x] Custom buttons per hive (replaces hardcoded "+ Agent")
- [x] CustomButtonsModal with previously-used suggestions from other hives
- [x] Fix: newly created hives not appearing in sidebar dropdown

## TODO — Short Term
- [ ] Setup screen: add onboarding guide / more info for first-time users
- [ ] Hive add: show a verification step (repo info preview before confirming)
- [ ] Hive add: show spinner/progress while gh verifies the repo
- [ ] Combs: test the full clone + branch workflow end to end
- [ ] Workspace: test terminal panes actually work (xterm.js + PTY)
- [ ] Workspace: keyboard shortcuts for pane management
- [ ] Workspace: resizable panes (drag to resize)

## TODO — Medium Term
- [ ] Environment management (.env files, mapping to combs)
- [x] Agent configuration per hive (which agent command to use) — done via custom buttons
- [ ] Comb: pull/push/sync with remote
- [ ] Comb: show git status in sidebar
- [x] Multiple agent support (claude, opencode, etc.) — done via custom buttons
- [x] Persist workspace pane layout per comb

## TODO — Long Term
- [ ] Setup screen onboarding wizard with illustrations
- [ ] Keyboard-driven navigation throughout (vim-style)
- [ ] Comb templates (pre-configured pane layouts)
- [ ] Shared secrets/config across combs
- [ ] Activity log / history
- [ ] Status dashboard (which agents are running, resource usage)

## Keyboard Interop

### How it works

Beehive uses the **kitty keyboard protocol** (CSI u) for reliable key encoding between
the outer terminal, Beehive's TUI/GUI, and inner applications (zellij, opencode, claude, etc.).

**CLI (TUI):**
- On startup, queries the outer terminal for keyboard enhancement support via `supports_keyboard_enhancement()`
- If supported, pushes `DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES` flags
- Key events from crossterm include SUPER/META modifiers and press/repeat/release kinds
- The `key_to_bytes` encoder translates these to proper CSI u / modified xterm sequences
- The `keyboard.rs` module detects when inner apps negotiate their own keyboard protocol
- When an inner app pushes keyboard enhancement (e.g. zellij sends `CSI > 1 u`), the encoder
  switches to full CSI u mode for ambiguous keys (Enter, Tab, Backspace, Esc)

**GUI (Tauri):**
- xterm.js 6.1+ with `vtExtensions.kittyKeyboard: true` handles protocol natively
- Inner apps can query and negotiate keyboard protocol directly with xterm.js
- The `attachCustomKeyEventHandler` only intercepts Cmd+C/V/A/Q for native macOS handling
- WebView keydown capture prevents browser from consuming Ctrl+Shift/Ctrl+Alt combos

### Environment variables

| Variable | Effect |
|----------|--------|
| `BEEHIVE_KEY_TRACE=1` | Logs every key event + encoded bytes to stderr (debug) |
| `BEEHIVE_FORCE_ENHANCED_KEYS=1` | Force CSI u encoding for all ambiguous keys, even if inner app didn't request it |

### Terminal requirements

For full modifier support (Cmd+A, Shift+Enter, etc.) the outer terminal must support the
kitty keyboard protocol. The Settings screen (`s` key) shows whether enhanced mode is active.

**Supported terminals:** WezTerm, Kitty, Alacritty, foot, Ghostty
**Partial support:** iTerm2 (CSI u but not full kitty stack)
**No support:** macOS Terminal.app (Cmd combos intercepted at app level — fundamental limitation)

### Validation matrix

| Scenario | Keys to test |
|----------|-------------|
| Direct shell | Ctrl+C, Ctrl+D, arrows, typing |
| Inside zellij | Ctrl+Shift+T (new tab), Ctrl+Shift+N (new pane) |
| Inside tmux | Ctrl+B prefix, Shift+arrows |
| OpenCode/Claude | Shift+Enter (newline), Cmd+A (select all), arrow keys |
| Nested (zellij in beehive) | All of the above |

## Design Notes
- Keep it Mac-first for now, cross-platform later
- Hive dirs: `repo_{reponame}` in the beehive directory
- Comb dirs: `{comb-name}/` inside the hive dir (full git clone)
- State lives in `.hive/state.json` per hive
- Terminals are free-form, not a fixed grid — add as many as you want
