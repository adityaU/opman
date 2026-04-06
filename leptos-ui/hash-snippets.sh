#!/usr/bin/env bash
# Post-build hook: content-hash JS assets in dist/ so CDN immutable caches
# always serve the correct version after rebuilds.
#
# Trunk gives files hashes based on the crate, not content. When we patch
# import paths (or content changes between builds with the same crate hash),
# the filename stays the same → CDN serves stale content → SRI mismatch.
#
# This script:
# 1. Renames snippet JS files to include a content hash
# 2. Patches import paths in the main JS bundle
# 3. Renames the main JS bundle to include a content hash
# 4. Updates all references and SRI hashes in index.html

set -euo pipefail

DIST="${TRUNK_STAGING_DIR:-dist}"
index="$DIST/index.html"

# ── Step 1: Content-hash snippet JS files ────────────────────────────
shopt -s nullglob
snippet_files=("$DIST"/snippets/*/*.js)
shopt -u nullglob

declare -A basemap  # old_basename -> new_basename

for f in "${snippet_files[@]}"; do
  dir="$(dirname "$f")"
  base="$(basename "$f")"
  name="${base%.js}"
  hash8="$(sha256sum "$f" | cut -c1-8)"
  new_base="${name}-${hash8}.js"
  mv "$f" "$dir/$new_base"
  basemap["$base"]="$new_base"
  echo "[hash-assets] snippet: $base -> $new_base"
done

# ── Step 2: Patch snippet paths in index.html ────────────────────────
if [ -f "$index" ] && [ ${#basemap[@]} -gt 0 ]; then
  for old_base in "${!basemap[@]}"; do
    new_base="${basemap[$old_base]}"
    sed -i "s|${old_base}|${new_base}|g" "$index"
  done
fi

# ── Step 3: Patch snippet paths in main JS bundle ────────────────────
shopt -s nullglob
bundles=("$DIST"/leptos-ui-*.js)
shopt -u nullglob

for bundle in "${bundles[@]}"; do
  if [ ${#basemap[@]} -gt 0 ]; then
    for old_base in "${!basemap[@]}"; do
      new_base="${basemap[$old_base]}"
      sed -i "s|${old_base}|${new_base}|g" "$bundle"
    done
  fi
done

# ── Step 4: Content-hash rename the main JS bundle ──────────────────
for bundle in "${bundles[@]}"; do
  old_name="$(basename "$bundle")"
  stem="${old_name%.js}"
  hash8="$(sha256sum "$bundle" | cut -c1-8)"
  new_name="${stem}-${hash8}.js"

  mv "$bundle" "$DIST/$new_name"
  echo "[hash-assets] bundle: $old_name -> $new_name"

  # Update all references in index.html (import path, modulepreload href)
  if [ -f "$index" ]; then
    sed -i "s|${old_name}|${new_name}|g" "$index"
  fi
done

# ── Step 5: Recalculate SRI hashes in index.html ────────────────────
# After renaming, we need to update integrity attributes for:
#   - The main JS bundle (content changed by import path patching + rename)
#   - Snippet files (already have correct hashes from trunk, paths just renamed)
if [ -f "$index" ]; then
  # Update main bundle SRI
  shopt -s nullglob
  new_bundles=("$DIST"/leptos-ui-*-*.js)
  shopt -u nullglob

  for bundle in "${new_bundles[@]}"; do
    bundle_name="$(basename "$bundle")"
    new_hash="sha384-$(openssl dgst -sha384 -binary "$bundle" | base64)"
    sed -i -E "s|(href=\"/${bundle_name}\"[^>]*integrity=\")sha384-[A-Za-z0-9+/=]+|\1${new_hash}|" "$index"
    echo "[hash-assets] SRI updated for $bundle_name"
  done
fi

echo "[hash-assets] Done."
