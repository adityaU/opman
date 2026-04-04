# opman

Terminal multiplexer and web UI wrapper for the [opencode](https://github.com/AnomalyAI/opencode) CLI. Manages multiple projects, sessions, and agents from a single interface — in the terminal (TUI) or browser (web UI).

## Features

- **Multi-project management** — switch between projects, each with independent sessions
- **Web UI** — browser-based chat interface with theme support (glassy / flat), mobile-friendly
- **Terminal multiplexer** — embedded terminal panes alongside the AI assistant
- **Session delegation** — spawn sub-agent sessions and track them from a central board
- **Code editor** — integrated editor with LSP support, diff review, and file explorer
- **Theming** — fully dynamic theme system with live-switching and PWA icon sync

## Requirements

- [Rust toolchain](https://rustup.rs/) (for building from source)
- [Node.js](https://nodejs.org/) ≥ 18 and npm (for the web UI frontend)
- [opencode](https://github.com/AnomalyAI/opencode) CLI installed and configured

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
