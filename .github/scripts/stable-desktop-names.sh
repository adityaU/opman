#!/usr/bin/env bash
# Copy every versioned desktop installer to a fixed name so the README download links
# (`.../releases/latest/download/<fixed name>`) never need editing.
#   opman-desktop-0.10.2-aarch64-apple-darwin.dmg -> latest/opman-desktop-latest-macos-arm64.dmg
# Usage: stable-desktop-names.sh <dir with versioned installers>
set -euo pipefail

dir="${1:?usage: stable-desktop-names.sh <dir>}"
out="$dir/latest"
mkdir -p "$out"

declare -A label=(
  [aarch64-apple-darwin]=macos-arm64
  [x86_64-apple-darwin]=macos-x64
  [x86_64-pc-windows-msvc]=windows-x64
  [x86_64-unknown-linux-gnu]=linux-x64
  [aarch64-unknown-linux-gnu]=linux-arm64
)

shopt -s nullglob
count=0
for target in "${!label[@]}"; do
  for file in "$dir"/opman-desktop-*-"$target"*; do
    name="${file##*/}"
    [[ "$name" == *.sha256 ]] && continue
    suffix="${name#opman-desktop-*-"$target"}" # ".dmg", "-setup.exe", ".AppImage", ...
    cp "$file" "$out/opman-desktop-latest-${label[$target]}$suffix"
    count=$((count + 1))
  done
done

if (( count == 0 )); then
  echo "::error::no desktop installers found in $dir"
  exit 1
fi
ls -1 "$out"
