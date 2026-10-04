# opman desktop

A [Tauri v2](https://v2.tauri.app) shell that packages opman as a native app for
macOS (Apple Silicon and Intel) and Windows (x86_64).

The app does not reimplement anything. It bundles the regular `opman` binary as a
sidecar, starts it with `--web-only --web-bind 127.0.0.1 --web-port <free port>`,
shows a small "Starting opman…" page (`ui/index.html`) until the port answers, then
points its window at `http://127.0.0.1:<port>/`. Quitting the app stops opman.

- opman's output goes to `opman.log` in the app log directory
  (`~/Library/Logs/dev.opman.desktop/` on macOS, `%LOCALAPPDATA%\dev.opman.desktop\logs\` on Windows).
  If opman fails to start, or stops later, the window shows the error and that path.
- macOS starts GUI apps with a minimal `PATH`, so the app asks your login shell
  (`$SHELL -ilc`) for its `PATH` and hands that to opman, letting it find `opencode`,
  `claude`, `git` and friends.

This crate is not a member of the root Cargo workspace and has its own `Cargo.lock`.

## Building locally

Prerequisites: Rust, Node 22, and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/)
for your OS.

```sh
# 1. Build the web UI and opman for your machine (from the repo root)
(cd web-ui && npm ci && npm run build)
cargo build --release

# 2. Place opman where the bundler expects it: binaries/opman-<target-triple>[.exe]
TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
cp target/release/opman "desktop/binaries/opman-$TRIPLE"   # add .exe on Windows

# 3. Build the app
cd desktop
npm ci
npx tauri build            # or: npx tauri dev
```

Installers land in `desktop/target/release/bundle/` (`.app`/`.dmg` on macOS,
NSIS `.exe` and `.msi` on Windows). `cargo check` in `desktop/` also needs the
sidecar file from step 2 to exist; an empty placeholder is enough.

Icons in `icons/` are generated from the web UI icon:
`npx tauri icon ../web-ui/public/icon-512.png -o icons`.

## CI

`.github/workflows/desktop.yml` builds the web UI once, then for each target builds
opman, stages it as the sidecar, and runs `tauri build` with the version read from
the root `Cargo.toml`, so the app always matches opman's version. Installers are
uploaded as `desktop-<target>` artifacts named `opman-desktop-<version>-<target>.*`
with `.sha256` files. It runs on pull requests touching `desktop/`, on demand, and
from `release.yml`, which attaches the installers to the GitHub release.

## Signing

Without secrets the macOS app is ad-hoc signed (`signingIdentity: "-"`) and not
notarized, and the Windows installers are unsigned:

- macOS: Gatekeeper will say the app "cannot be opened" or is "damaged". Right-click
  the app and choose Open, or run `xattr -dr com.apple.quarantine /Applications/opman.app`.
- Windows: SmartScreen shows "Windows protected your PC"; choose More info, then Run anyway.

To sign and notarize on macOS, set the repository secrets `APPLE_CERTIFICATE`
(base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, and for notarization
`APPLE_ID` + `APPLE_PASSWORD` (app-specific password) + `APPLE_TEAM_ID`. The workflow
only exports the ones that are set.
