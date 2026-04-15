# Beehive

Orchestrate coding agents across isolated git workspaces.

**[Website](https://www.beehiveapp.dev)** | **[Download](https://github.com/storozhenko98/beehive/releases/latest)** | **[Install TUI](#tui)**

Beehive lets you manage multiple repos, create isolated workspace clones on different branches, and run terminals and AI agents side-by-side — all from one window. Available as a desktop GUI app and a terminal TUI.

## Install

### Desktop App (macOS)

Download the latest `.dmg` from [Releases](https://github.com/storozhenko98/beehive/releases/latest). Signed and notarized for Apple Silicon.

### TUI

macOS and Linux `x86_64` are supported for the TUI.

Install directly:

```bash
curl -fsSL beehiveapp.dev/install.sh | bash
```

You'll be asked to choose `bh` or `beehive` as your command name. Auto-updates on startup.

You can also set the comb startup command programmatically from the TUI binary:

```bash
bh --startup-cmd 'tmux new-session -A -s "$(basename "$BEEHIVE_COMB")"'
```

Clear it with:

```bash
bh --startup-cmd ''
```

Or download the latest standalone TUI binary from [Releases](https://github.com/storozhenko98/beehive/releases/latest):

- macOS: `beehive-tui-darwin-arm64`
- Linux x64: `beehive-tui-linux-x64`

## Features

- **Multi-repo management** — Add GitHub repositories and switch between them instantly
- **Isolated workspaces** — Full git clones ("combs") on any branch, each in its own directory
- **Persistent terminals** — Real PTY sessions that stay alive across workspace and repo switches
- **Agent panes** — Launch Claude Code or any CLI agent alongside your terminals
- **Copy combs** — Duplicate a workspace including uncommitted work to experiment safely
- **Custom buttons** — Configure per-repo quick-launch buttons for your agent commands
- **Comb startup command** — Run a configurable shell command automatically the first time each comb opens after launch
- **Live branch tracking** — Sidebar updates when you switch branches in the terminal
- **Layout persistence** — Pane layouts saved to disk and restored on restart
- **Resizable sidebar** (TUI) — `<`/`>` keys to adjust, persisted across sessions

## Concepts

| Term | What it is |
|------|-----------|
| **Hive** | A linked GitHub repository |
| **Comb** | An isolated workspace clone of a hive, on a specific branch |
| **Pane** | A terminal or agent session inside a workspace |

## How It Works

1. **Add a hive** — Link a GitHub repo by URL
2. **Create a comb** — Clone it to an isolated directory on any branch
3. **Open panes** — Launch terminals and agents side-by-side
4. **Switch freely** — All terminals persist in the background across workspace and hive switches

## Prerequisites

| Tool | Required for | Install |
|------|-------------|---------|
| **git** | All | [git-scm.com](https://git-scm.com/) |
| **GitHub CLI** (`gh`) | All | [cli.github.com](https://cli.github.com/) |
| **Node.js** 18+ | Building GUI | [nodejs.org](https://nodejs.org/) |
| **Rust** (stable) | Building GUI/TUI | [rustup.rs](https://rustup.rs/) |

```bash
gh auth login   # Required — Beehive uses gh for repo operations
```

> **Note:** The desktop GUI is macOS-only for now. The standalone TUI supports macOS (Apple Silicon) and Linux x64. Windows support is not available yet.

## Building from Source

### Desktop GUI

```bash
git clone https://github.com/storozhenko98/beehive.git
cd beehive
npm install
npm run tauri dev        # Development with hot-reload
npm run tauri build      # Production .app and .dmg
```

Or use the build script:

```bash
./build.sh               # Production build
./build.sh --dev          # Dev mode
./build.sh --check        # Type-check only
```

### TUI

```bash
cd cli
source "$HOME/.cargo/env"
cargo build --release
# Binary at cli/target/release/beehive-tui
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| GUI Frontend | React 19, TypeScript 5.8, Vite 7, xterm.js 5.5 |
| GUI Backend | Rust, Tauri v2, portable-pty 0.9 |
| TUI | Rust, Ratatui 0.29, Crossterm 0.28, portable-pty, vt100 |
| Website | Next.js 16, Tailwind, shadcn/ui |
| Styling | Catppuccin Mocha theme throughout |
| Git ops | `git` and `gh` CLI via `std::process::Command` |

## Project Structure

```
beehive/
├── src/                    # React frontend (GUI)
│   ├── App.tsx             # Screen router
│   ├── App.css             # Styles (Catppuccin Mocha)
│   ├── types.ts            # Shared TypeScript types
│   └── components/         # UI components (12 files)
├── src-tauri/              # Rust backend (GUI)
│   └── src/
│       ├── lib.rs          # Tauri app builder
│       ├── pty.rs          # PTY session management
│       └── hive.rs         # Hive/comb CRUD, git operations
├── cli/                    # TUI (standalone Rust binary)
│   └── src/
│       ├── main.rs         # Event loop, key handling
│       ├── ui.rs           # Ratatui rendering
│       ├── app.rs          # Application state
│       ├── terminal.rs     # Embedded PTY (portable-pty + vt100)
│       ├── config.rs       # Config/state management
│       └── update.rs       # Auto-update
├── web/                    # Website (beehiveapp.dev)
│   └── src/app/            # Next.js app router
├── docs/                   # GitHub Pages landing (legacy)
├── .github/workflows/      # CI: release pipeline + web deploy
├── install.sh              # TUI install script
├── build.sh                # GUI build script
└── CLAUDE.md               # AI assistant context
```

## Config & Data

All config lives in `~/.beehive/`:

| File | Purpose |
|------|---------|
| `~/.beehive/config.json` | App config (beehive directory path, sidebar width, CLI preferences, comb startup command) |
| `{beehiveDir}/beehive.json` | Directory marker with version |
| `{beehiveDir}/repo_{name}/.hive/state.json` | Hive state (repo info, combs, pane layouts, custom buttons) |

Combs are full git clones at `{beehiveDir}/repo_{name}/{combName}/`.

The GUI and TUI share the same config and data — you can use both interchangeably.

Example startup command in `~/.beehive/config.json`:

```json
{
  "combStartupCommand": "tmux new-session -A -s \"$(basename \"$BEEHIVE_COMB\")\""
}
```

Beehive runs that command once per comb the first time it opens that comb after launch, then returns to an interactive shell in the comb directory.

## Platform Support

| Platform | GUI | TUI |
|----------|-----|-----|
| **macOS** (Apple Silicon) | Signed & notarized | Supported |
| **macOS** (Intel) | Should work (untested) | Should work (untested) |
| **Linux** | Not yet | Supported (`x86_64` glibc) |
| **Windows** | Not yet | Not yet |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

[Open an issue](https://github.com/storozhenko98/beehive/issues/new) for bugs or feature requests.

## Author

**Mykyta Storozhenko** — [storozh.dev](https://storozh.dev) · [@storozhenko98](https://github.com/storozhenko98) · [@technoleviathan](https://x.com/technoleviathan)

## License

[MIT](LICENSE)
