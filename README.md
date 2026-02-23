# Beehive

**Orchestrate coding agents across isolated git workspaces.**

Beehive is a desktop application that helps you manage multiple coding agents working on the same repository in parallel. Each agent gets its own isolated git workspace (a full clone on a specific branch), with a dedicated terminal pane — all managed from a single window.

## Concepts

- **Beehive** — your root working directory where everything lives
- **Hive** — a linked GitHub repository
- **Comb** — an isolated workspace clone of a hive, checked out to a specific branch
- **Terminal** — a PTY pane inside a workspace where you run agents or commands

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18+)
- [git](https://git-scm.com/)
- [GitHub CLI (`gh`)](https://cli.github.com/) — must be authenticated (`gh auth login`)

## Setup

```bash
# Clone this repo
git clone <repo-url> beehive
cd beehive

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev
```

On first launch, Beehive will:
1. Check that `git`, `gh`, and `gh auth` are available
2. Ask you to pick a directory for your beehive
3. Show the hive list where you can add repositories

## Architecture

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Frontend | React + TypeScript + Vite | UI screens, terminal rendering |
| Terminal | xterm.js + FitAddon | Terminal emulation in the browser |
| Backend | Rust + Tauri v2 | PTY management, git operations, file I/O |
| PTY | portable-pty | Real pseudo-terminal sessions |
| Git | git/gh CLI via std::process::Command | Repository and branch operations |

### Screen Flow

```
Preflight → Setup → Hive List → Comb List → Workspace
              ↑                                  │
              └──── Settings (reset) ←───────────┘
```

### Data Flow (Terminal)

```
User keypress → xterm.js onData → invoke("write_to_pty") → PTY stdin
PTY stdout → background reader thread → emit("pty-output-{id}") → listen() → xterm.js write
```

## Project Structure

```
src/                    React frontend
src/components/         Screen components + TerminalPane
src-tauri/src/          Rust backend (lib.rs, pty.rs, hive.rs)
plan.md                 Development TODO and design notes
```

## Status

Early development. Core features (preflight, hive management, comb management, terminal panes) are implemented. See `plan.md` for the roadmap.

## License

Private — all rights reserved.
