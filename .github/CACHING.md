# Build caches

GitHub only restores caches from the current ref and the default branch. A new
release tag cannot restore caches saved by an older release tag, even with a
matching restore key.

The Warm build caches workflow therefore runs on `master`, the repository's
default branch. It calls the reusable Build release artifacts workflow on `develop`, without
inheriting secrets, and builds `develop` while keeping caches scoped to `master`.
The frontend resolves the source commit once; all platform jobs use that exact
commit. This keeps caches current without merging application history into master.

Keep the reusable Build release artifacts and Desktop workflows on `develop`, and install the
small `warm-caches.yml` workflow on `master`. Let a master warm-up finish before
expecting release tags to restore its caches. A manual Warm build caches run on
master can refresh them at any time. Keeping caches only on develop or release
tags does not make them available to a new tag.

## What is reused

- Node setup caches npm package downloads using each project's lockfile. `npm ci`
  still runs so dependency installation stays reproducible.
- The release builds the web UI once. Backend and desktop jobs download the same
  frontend artifact. Standalone desktop runs still build their own frontend.
- The pinned Rust cache action caches registry downloads and compiled dependencies
  for each runner and target. It keys caches by toolchain, build environment and
  dependency manifests, and can restore compatible dependencies after lockfile
  changes. Desktop builds include both Cargo workspaces.
- Workspace binaries, unused crates and incremental artifacts are not retained.
  This reduces cache size and prevents an old embedded frontend from being reused.
- Pull requests may restore caches, but do not save Rust caches.
- Zig setup keeps its existing download/build caching.

Backend and desktop Rust caches remain separate: their workspaces and compilation
environments differ. Architecture-specific build outputs must not be mixed.
Final application compilation, linking and installer packaging still run.

## Checking a run

Look for cache restore/save messages under Node setup and Cache Rust dependencies.
An initial default-branch build is expected to miss; later tags should restore
from it. GitHub can evict old caches when storage limits are reached, so cache
misses must never affect build correctness.

Only version-tag pushes publish GitHub releases and receive Apple signing secrets.
Manual warm-ups do neither, even if a tag is selected. Default-branch warm-up runs
have read-only repository permissions; only the publish job receives release
write permission.

The reusable build workflow has read-only permissions throughout. Publishing is
separate, so a warm-up cannot request release write access through a nested job.
