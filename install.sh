#!/usr/bin/env bash
# install.sh — Download the latest opman release and install to ~/.local/bin
set -euo pipefail

REPO="adityaU/opman"
BINARY_NAME="opman"
INSTALL_DIR="$HOME/.local/bin"

# ── Colour helpers ──────────────────────────────────────────────
red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
dim()   { printf '\033[0;90m%s\033[0m\n' "$*"; }

# ── Platform detection ──────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  OS_TAG="unknown-linux-gnu" ;;
  Darwin) OS_TAG="apple-darwin"      ;;
  *) red "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  aarch64|arm64) ARCH_TAG="aarch64" ;;
  x86_64)        ARCH_TAG="x86_64"  ;;
  *) red "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ASSET_NAME="opman-${ARCH_TAG}-${OS_TAG}.tar.gz"

# ── Fetch latest release URL ───────────────────────────────────
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
dim "Fetching latest release from $REPO..."

if command -v curl &>/dev/null; then
  RELEASE_JSON="$(curl -fsSL "$API_URL")"
elif command -v wget &>/dev/null; then
  RELEASE_JSON="$(wget -qO- "$API_URL")"
else
  red "Neither curl nor wget found. Install one and retry."
  exit 1
fi

# Extract download URL for the matching asset
DOWNLOAD_URL="$(printf '%s' "$RELEASE_JSON" \
  | grep -o "\"browser_download_url\": *\"[^\"]*${ASSET_NAME}\"" \
  | head -1 \
  | sed 's/.*"browser_download_url": *"//' \
  | sed 's/"$//')"

if [ -z "$DOWNLOAD_URL" ]; then
  red "No release asset found matching $ASSET_NAME"
  dim "Available assets:"
  printf '%s' "$RELEASE_JSON" \
    | grep '"browser_download_url"' \
    | sed 's/.*"browser_download_url": *"/  /' \
    | sed 's/"$//'
  exit 1
fi

VERSION="$(printf '%s' "$RELEASE_JSON" \
  | grep '"tag_name"' \
  | head -1 \
  | sed 's/.*"tag_name": *"//' \
  | sed 's/".*//')"
green "Latest release: $VERSION"

# ── Download & extract ─────────────────────────────────────────
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

dim "Downloading $ASSET_NAME..."
if command -v curl &>/dev/null; then
  curl -fsSL -o "$TMP_DIR/$ASSET_NAME" "$DOWNLOAD_URL"
else
  wget -qO "$TMP_DIR/$ASSET_NAME" "$DOWNLOAD_URL"
fi

tar xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"

if [ ! -f "$TMP_DIR/$BINARY_NAME" ]; then
  red "Expected binary '$BINARY_NAME' not found in archive"
  exit 1
fi

# ── Install binary ─────────────────────────────────────────────
mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
green "Installed $BINARY_NAME $VERSION to $INSTALL_DIR/$BINARY_NAME"

# ── Ensure ~/.local/bin is on PATH ─────────────────────────────
update_rc() {
  local rc_file="$1"
  local path_line='export PATH="$HOME/.local/bin:$PATH"'

  [ -f "$rc_file" ] || return 1

  if grep -qF '.local/bin' "$rc_file" 2>/dev/null; then
    dim "$rc_file already has ~/.local/bin in PATH"
    return 0
  fi

  printf '\n# Added by opman installer\n%s\n' "$path_line" >> "$rc_file"
  green "Updated $rc_file with PATH entry"
}

CURRENT_SHELL="$(basename "${SHELL:-/bin/bash}")"
PATH_UPDATED=false

case "$CURRENT_SHELL" in
  zsh)
    update_rc "$HOME/.zshrc" && PATH_UPDATED=true
    ;;
  bash)
    if [ "$OS" = "Darwin" ]; then
      update_rc "$HOME/.bash_profile" && PATH_UPDATED=true
    else
      update_rc "$HOME/.bashrc" && PATH_UPDATED=true
    fi
    ;;
  fish)
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
    update_rc "$HOME/.bashrc" && PATH_UPDATED=true
    update_rc "$HOME/.zshrc"  && PATH_UPDATED=true
    ;;
esac

# ── Verify ─────────────────────────────────────────────────────
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
