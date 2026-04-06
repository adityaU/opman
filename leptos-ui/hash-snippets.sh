#!/usr/bin/env bash
# Post-build hook: content-hash inline JS snippet files in dist/snippets/
# so that CDN caches (max-age=immutable) serve the correct version after rebuilds.
#
# Trunk gives snippet files stable names (inline0.js, inline1.js) that don't
# change when content changes. This script renames them to include a short
# content hash (e.g. inline0-a1b2c3d4.js) and patches all references in
# dist/index.html and the main JS bundle.

set -euo pipefail

DIST="${TRUNK_STAGING_DIR:-dist}"

# Find all JS files under dist/snippets/
shopt -s nullglob
snippet_files=("$DIST"/snippets/*/*.js)
shopt -u nullglob

if [ ${#snippet_files[@]} -eq 0 ]; then
  echo "[hash-snippets] No snippet JS files found, skipping."
  exit 0
fi

# Collect rename pairs: old_basename -> new_basename
declare -A renames  # old_path -> new_path
declare -A basemap  # old_basename -> new_basename

for f in "${snippet_files[@]}"; do
  dir="$(dirname "$f")"
  base="$(basename "$f")"
  name="${base%.js}"

  # 8-char content hash
  hash8="$(sha256sum "$f" | cut -c1-8)"
  new_base="${name}-${hash8}.js"

  mv "$f" "$dir/$new_base"
  renames["$f"]="$dir/$new_base"
  basemap["$base"]="$new_base"

  echo "[hash-snippets] $base -> $new_base"
done

# Patch dist/index.html — replace old snippet paths with new ones
index="$DIST/index.html"
if [ -f "$index" ]; then
  for old_base in "${!basemap[@]}"; do
    new_base="${basemap[$old_base]}"
    sed -i "s|${old_base}|${new_base}|g" "$index"
  done
  echo "[hash-snippets] Patched index.html"
fi

# Patch the main JS bundle — replace import paths
shopt -s nullglob
bundles=("$DIST"/leptos-ui-*.js)
shopt -u nullglob

for bundle in "${bundles[@]}"; do
  for old_base in "${!basemap[@]}"; do
    new_base="${basemap[$old_base]}"
    sed -i "s|${old_base}|${new_base}|g" "$bundle"
  done
  echo "[hash-snippets] Patched $(basename "$bundle")"
done

# Recalculate SRI hashes in index.html for the main bundle (since we changed its content)
if [ -f "$index" ]; then
  for bundle in "${bundles[@]}"; do
    bundle_name="$(basename "$bundle")"
    new_hash="sha384-$(openssl dgst -sha384 -binary "$bundle" | base64)"
    # Match the integrity attribute for this bundle's link tag
    # Pattern: href="/bundle-name" ... integrity="sha384-..."
    sed -i -E "s|(href=\"/${bundle_name}\"[^>]*integrity=\")sha384-[A-Za-z0-9+/=]+|\1${new_hash}|" "$index"
  done
  echo "[hash-snippets] Updated SRI hashes in index.html"
fi

echo "[hash-snippets] Done."
