# opman

Terminal multiplexer and web UI wrapper for the [opencode](https://github.com/AnomalyAI/opencode) CLI. Manages multiple projects, sessions, and agents from a single interface — in the terminal (TUI) or browser (web UI).

## Features

- **Multi-project management** — switch between projects, each with independent sessions
- **Web UI** — browser-based chat interface with theme support (glassy / flat), mobile-friendly
- **Terminal multiplexer** — embedded terminal panes alongside the AI assistant
- **Session delegation** — spawn sub-agent sessions and track them from a central board
- **Code editor** — integrated editor with LSP support, diff review, and file explorer
- **Theming** — fully dynamic theme system with live-switching and PWA icon sync

## Prerequisites

### Required

| Tool | Why |
|------|-----|
| [opencode](https://github.com/AnomalyAI/opencode) | Core dependency — opman is a wrapper around the opencode CLI |
| [git](https://git-scm.com/) | Used for diffs, branches, commits, and the built-in git panel |
| A POSIX shell (`$SHELL` or `/bin/bash`) | Powers the integrated terminal panes |

### Optional (feature-dependent)

| Tool | When needed |
|------|-------------|
| [Neovim](https://neovim.io/) (`nvim`) | Editor pane and Neovim MCP bridge (`--neovim-mcp`) |
| [gitui](https://github.com/extrawurst/gitui) | Git panel TUI (spawned inside a terminal pane) |
| [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) | Exposing the web UI via a Cloudflare tunnel (`--tunnel`) |
| [Docker](https://docs.docker.com/get-docker/) + Compose | Preflight container checks (e.g. SearXNG search) — gracefully skipped if absent |
| `pbcopy` (macOS) or `xclip` (Linux) | Clipboard support in TUI mode |
| `alacritty` or `wezterm` | Popout terminal panels (auto-detected via `which`) |

### Building from source

| Tool | Why |
|------|-----|
| [Rust toolchain](https://rustup.rs/) (stable ≥ 1.94) | Compiles the opman binary |
| `wasm32-unknown-unknown` target | Leptos frontend compiles to WASM — install with `rustup target add wasm32-unknown-unknown` |
| [Trunk](https://trunkrs.dev/) | WASM bundler for the Leptos web UI — install with `cargo install trunk --locked` |
| [Node.js](https://nodejs.org/) ≥ 18 + npm | Runs `npx tailwindcss` during the Trunk build |

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/adityaU/opman/master/install.sh | bash
```

This downloads the latest release binary for your platform and installs it to `~/.local/bin`.

### From source (Linux / macOS)

```bash
git clone https://github.com/adityaU/opman.git
cd opman

# Build frontend + backend
cd leptos-ui && npm install && cd .. 
cd leptos-ui && npx trunk build --release && cd ..
cargo build --release

# Install to ~/.local/bin and update PATH
./install.sh
```

### Manual install

```bash
# Build as above, then copy the binary yourself:
cp target/release/opman ~/.local/bin/
```

Ensure `~/.local/bin` is in your `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

## Usage

```bash
# Start opman (TUI mode)
opman

# Start with web UI on a specific port
opman --web-port 8080
```

## License

Private — all rights reserved.
