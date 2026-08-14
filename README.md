# opman

A coding-agent workbench. opman runs your AI coding agents, your editor, your terminals, and your git workflow behind one interface — in the terminal (TUI) or in the browser.

It started as a wrapper around a single CLI. It is now runner-agnostic: any agent that speaks the [Agent Client Protocol](https://agentclientprotocol.com/) plugs in through config, alongside first-class support for [opencode](https://github.com/AnomalyAI/opencode), Claude Code, and Codex.

## What it does

### Agents

- **Multiple runners, one UI** — opencode, Claude Code, and any ACP agent share the same chat, tool cards, permission prompts, and history. 39 ACP harnesses are declared in the catalog; `claude` and `codex` ship enabled, the rest are one config edit away.
- **Per-session engine** — runner, model, agent/mode, and reasoning effort are chosen per session from a single merged picker. Switching runners mid-thread hands off a real transcript instead of a summary.
- **Add agents from settings** — ACP agents are added, configured, and removed at runtime from `/settings`; the supervisor reconciles running processes without a restart.
- **Slash commands from the runner** — the command list is asked of whichever runner owns the session, then executed on it.
- **Sub-agents** — spawned sessions render inline and can be opened into their own pane.

### MCP servers, built in

opman ships its own MCP servers and injects them into every runner from one registry (`mcp.json`):

| Server | What it gives the agent |
|---|---|
| `agent-manager` | start, message, wait on, and abort other agent sessions across runners |
| `ask` | multiple-choice questions to the user, mid-turn |
| `ui` | rich UI blocks (cards, tables, charts, diffs) rendered inline in chat |
| `kanban` | read and move tasks on the built-in board |
| `skills` | list and load skills |
| `neovim` | drive the embedded Neovim |
| `time`, `probe`, `proxy`, `oauth` | clock, health checks, upstream MCP proxying, OAuth flows |

The settings page launches each configured server and lists its real tools and schemas.

### Workspace

- **Tmux-style panes** — windows, splits, and widgets. Terminals, editors, chat, git, and browser panes live side by side, with per-pane engines, PTY scrollback replay, and a Zen mode.
- **Code editor** — native Rust LSP (no Neovim required): completion, diagnostics, hover, go-to-definition. Diff review, file explorer, and git panel included.
- **Neovim integration** — an embedded Neovim owns the keyboard when focused, with real grid rendering for splits and floats.
- **Terminals** — full PTY panes, mobile key bar, popout to alacritty/wezterm.

### Shell

- **Two-mode keybindings** — layered base/platform/target keymap spec in `keybindings.json`, with which-key discovery and live shortcut hints everywhere a chord is displayed.
- **Theming** — glassy and flat variants, live switching, generated palettes, PWA icon sync.
- **Mobile** — a distinct mobile layout rather than a squeezed desktop one.
- **Settings page** — one `/settings` route covering engines, MCP servers, keybindings, themes, and secrets.

## Prerequisites

### Required

| Tool | Why |
|------|-----|
| At least one agent runner — [opencode](https://github.com/AnomalyAI/opencode), Claude Code, Codex, or any ACP agent | opman drives agents; it doesn't ship one |
| [git](https://git-scm.com/) | Used for diffs, branches, commits, and the built-in git panel |
| A POSIX shell (`$SHELL` or `/bin/bash`) | Powers the integrated terminal panes |

### Optional (feature-dependent)

| Tool | When needed |
|------|-------------|
| [Neovim](https://neovim.io/) (`nvim`) | Neovim editor pane and MCP bridge (`--neovim-mcp`) |
| [Node.js](https://nodejs.org/) / [uv](https://docs.astral.sh/uv/) | Launching ACP agents distributed as npx/uvx packages |
| [gitui](https://github.com/extrawurst/gitui) | Git panel TUI (spawned inside a terminal pane) |
| [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) | Exposing the web UI via a Cloudflare tunnel (`--tunnel`) |
| [Docker](https://docs.docker.com/get-docker/) + Compose | Preflight container checks (e.g. SearXNG search) — gracefully skipped if absent |
| `pbcopy` (macOS) or `xclip` (Linux) | Clipboard support in TUI mode |
| `alacritty` or `wezterm` | Popout terminal panels (auto-detected via `which`) |

### Building from source

| Tool | Why |
|------|-----|
| [Rust toolchain](https://rustup.rs/) (stable ≥ 1.94) | Compiles the opman binary |
| [Node.js](https://nodejs.org/) ≥ 18 + npm | Builds the React web UI with Vite |

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/adityaU/opman/develop/install.sh | bash
```

This downloads the latest release binary for your platform and installs it to `~/.local/bin`.

### From source (Linux / macOS)

```bash
git clone https://github.com/adityaU/opman.git
cd opman

# Build frontend + backend
cd web-ui && npm install && npm run build && cd ..
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

## Configuration

Config lives under `~/.config/opman/`:

| File | Contents |
|---|---|
| `acp.json` | ACP agents: which are enabled, how each is launched, per-agent settings |
| `mcp.json` | MCP servers injected into every runner |
| `keybindings.json` | Keymap overrides on top of the built-in layers |

Most of this is editable from the `/settings` page; the files are the source of truth.

## Docs

- [`docs/acp-agents.md`](docs/acp-agents.md) — adding and configuring ACP agents
- [`docs/workspace-panes-design.md`](docs/workspace-panes-design.md) — pane/window model
- [`docs/BUG-agent-manager-socket.md`](docs/BUG-agent-manager-socket.md) — MCP socket lifetime gotcha

## License

Private — all rights reserved.
