#!/usr/bin/env bash
# install.sh — Install opman binary to ~/.local/bin and update PATH
set -euo pipefail

BINARY_NAME="opman"
INSTALL_DIR="$HOME/.local/bin"
RELEASE_BIN="target/release/$BINARY_NAME"

# ── Colour helpers ──────────────────────────────────────────────
red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
dim()   { printf '\033[0;90m%s\033[0m\n' "$*"; }

# ── Pre-flight checks ──────────────────────────────────────────
OS="$(uname -s)"
case "$OS" in
  Linux|Darwin) ;;
  *) red "Unsupported OS: $OS (only Linux and macOS are supported)"; exit 1 ;;
esac

if [ ! -f "$RELEASE_BIN" ]; then
  echo "Release binary not found at $RELEASE_BIN"
  echo "Building in release mode..."
  cargo build --release
fi

# ── Install binary ──────────────────────────────────────────────
mkdir -p "$INSTALL_DIR"
cp "$RELEASE_BIN" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
green "Installed $BINARY_NAME to $INSTALL_DIR/$BINARY_NAME"

# ── Ensure ~/.local/bin is on PATH ──────────────────────────────
update_rc() {
  local rc_file="$1"
  local path_line='export PATH="$HOME/.local/bin:$PATH"'

  if [ ! -f "$rc_file" ]; then
    return 1
  fi

  if grep -qF '.local/bin' "$rc_file" 2>/dev/null; then
    dim "$rc_file already has ~/.local/bin in PATH"
    return 0
  fi

  printf '\n# Added by opman installer\n%s\n' "$path_line" >> "$rc_file"
  green "Updated $rc_file with PATH entry"
  return 0
}

# Detect shell and update the appropriate rc file
CURRENT_SHELL="$(basename "${SHELL:-/bin/bash}")"
PATH_UPDATED=false

case "$CURRENT_SHELL" in
  zsh)
    update_rc "$HOME/.zshrc" && PATH_UPDATED=true
    ;;
  bash)
    # macOS uses .bash_profile, Linux uses .bashrc
    if [ "$OS" = "Darwin" ]; then
      update_rc "$HOME/.bash_profile" && PATH_UPDATED=true
    else
      update_rc "$HOME/.bashrc" && PATH_UPDATED=true
    fi
    ;;
  fish)
    # fish uses a different syntax
    FISH_CONFIG="$HOME/.config/fish/config.fish"
    if [ -f "$FISH_CONFIG" ] && grep -qF '.local/bin' "$FISH_CONFIG" 2>/dev/null; then
      dim "$FISH_CONFIG already has ~/.local/bin in PATH"
      PATH_UPDATED=true
    elif [ -f "$FISH_CONFIG" ]; then
      printf '\n# Added by opman installer\nfish_add_path $HOME/.local/bin\n' >> "$FISH_CONFIG"
      green "Updated $FISH_CONFIG with PATH entry"
      PATH_UPDATED=true
    fi
    ;;
  *)
    # Try both common rc files
    update_rc "$HOME/.bashrc" && PATH_UPDATED=true
    update_rc "$HOME/.zshrc"  && PATH_UPDATED=true
    ;;
esac

# ── Verify ──────────────────────────────────────────────────────
echo ""
if command -v "$BINARY_NAME" &>/dev/null; then
  green "opman is ready! Run 'opman' to start."
else
  if [ "$PATH_UPDATED" = true ]; then
    echo "Restart your shell or run:  source ~/.${CURRENT_SHELL}rc"
  else
    echo "Add ~/.local/bin to your PATH manually:"
    dim "  export PATH=\"\$HOME/.local/bin:\$PATH\""
  fi
  echo ""
  green "Then run 'opman' to start."
fi
