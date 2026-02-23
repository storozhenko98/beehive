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

## Design Notes
- Keep it Mac-first for now, cross-platform later
- Hive dirs: `repo_{reponame}` in the beehive directory
- Comb dirs: `{comb-name}/` inside the hive dir (full git clone)
- State lives in `.hive/state.json` per hive
- Terminals are free-form, not a fixed grid — add as many as you want
