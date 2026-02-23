# Beehive

Orchestrate coding agents across isolated git workspaces.

Beehive is a desktop app that lets you manage multiple repositories, create isolated workspace clones on different branches, and run terminal and AI agent panes side-by-side — all from a single window. Switch between projects without losing terminal state.

## Features

- **Multi-repo management** — Add GitHub repositories and manage them from one place
- **Isolated workspaces** — Create full git clones ("combs") on any branch, each in its own directory
- **Persistent terminals** — Real PTY sessions that stay alive when you switch between workspaces
- **Agent panes** — Launch Claude Code (or any CLI tool) alongside your terminals
- **Copy combs** — Duplicate a workspace including uncommitted work to experiment safely
- **Flexible grid** — Add/remove terminal and agent panes per workspace
- **Layout persistence** — Pane layouts are saved to disk and restored on restart

## Concepts

| Term | What it is |
|------|-----------|
| **Beehive** | Your root working directory where everything is stored |
| **Hive** | A linked GitHub repository |
| **Comb** | An isolated workspace clone of a hive, checked out to a specific branch |
| **Pane** | A terminal or agent session inside a workspace |

## Prerequisites

You need the following installed before building or running Beehive:

| Tool | Version | What for | Install |
|------|---------|----------|---------|
| **Node.js** | 18+ | Frontend build | [nodejs.org](https://nodejs.org/) |
| **Rust** | stable (latest) | Backend build | [rustup.rs](https://rustup.rs/) |
| **git** | any recent | Workspace operations | [git-scm.com](https://git-scm.com/) |
| **GitHub CLI** (`gh`) | any recent | Repo management | [cli.github.com](https://cli.github.com/) |

The GitHub CLI must be authenticated:

```bash
gh auth login
```

### macOS

```bash
xcode-select --install
```

> **Note:** Beehive currently only supports macOS. Linux and Windows support is planned — contributions welcome! See [Contributing](#contributing).

## Quick Start

```bash
git clone https://github.com/YOUR_USERNAME/beehive.git
cd beehive
npm install
npm run tauri dev
```

## Building from Source

The included build script handles everything:

```bash
# Production build (outputs .app and .dmg)
./build.sh

# Development mode with hot-reload
./build.sh --dev

# Type-check only (TypeScript + Rust, no build)
./build.sh --check
```

### Manual build

If you prefer to run the steps yourself:

```bash
# Install dependencies
npm install

# Type-check
npx tsc --noEmit
source "$HOME/.cargo/env" && cd src-tauri && cargo check && cd ..

# Production build
npm run tauri build
```

Build outputs:
- `src-tauri/target/release/bundle/macos/Beehive.app`
- `src-tauri/target/release/bundle/dmg/Beehive_<version>_aarch64.dmg`

### Install

```bash
cp -r src-tauri/target/release/bundle/macos/Beehive.app /Applications/
```

Or open the `.dmg` and drag to Applications.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19, TypeScript 5.8, Vite 7 |
| Terminal | xterm.js 5.5 (Unicode 11, web links, fit addon) |
| Backend | Rust, Tauri v2 |
| PTY | portable-pty 0.9 |
| Styling | CSS with Catppuccin Mocha theme |
| Git ops | `git` and `gh` CLI via `std::process::Command` |

## Project Structure

```
beehive/
├── src/                        # React frontend
│   ├── App.tsx                 # Screen router
│   ├── App.css                 # Styles (Catppuccin Mocha)
│   ├── types.ts                # Shared TypeScript types
│   └── components/             # UI components
├── src-tauri/
│   └── src/
│       ├── lib.rs              # Tauri app builder
│       ├── pty.rs              # PTY session management
│       └── hive.rs             # Hive/comb CRUD, git operations
├── public/                     # Static assets (logo)
├── build.sh                    # Build script
├── CONTRIBUTING.md             # Contributor guide
├── LICENSE                     # MIT License
└── CLAUDE.md                   # AI assistant context
```

## How It Works

1. **Add a hive** — Register a GitHub repo by URL
2. **Create a comb** — Clone it to an isolated directory on a specific branch
3. **Open panes** — Launch terminals and AI agents side-by-side
4. **Switch freely** — All terminals persist in the background across workspace and hive switches

Data flow for terminal I/O:

```
Keypress → xterm.js → invoke("write_to_pty") → PTY stdin
PTY stdout → background thread → emit("pty-output-{id}") → xterm.js
```

## Platform Support

| Platform | Status |
|----------|--------|
| **macOS** (Apple Silicon & Intel) | Supported |
| **Linux** | Not yet — contributions welcome |
| **Windows** | Not yet — contributions welcome |

Beehive is built on Tauri v2, which supports all three platforms. The app hasn't been tested or built on Linux/Windows yet. If you'd like to help, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, project structure, and guidelines.

## License

[MIT](LICENSE)
