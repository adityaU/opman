# Remaining work: the MCP + skills effort

Task 8 — the settings **page** — is built. `cargo test` is 2736 passing, `cargo clippy
--all-targets` is clean for the new code, and the web build and vitest suite pass (the four
`api.test.ts` failures predate this work; they assert the `Authorization: Bearer` header
that cookie auth replaced).

Nothing is committed. `git status` shows the whole change set.

---

## What Task 8 turned into

`/settings` is a path-based destination with four sections — Appearance, Keybindings, MCP
Servers, Skills — reached from the palette, the keymap (`mod+,`, `mod+k mod+s`, `mod+k
mod+p`, `mod+k mod+l`, and the vim leader's `<leader>u` group) and the slash commands
`/settings`, `/theme`, `/keys`, `/mcp`, `/skills`.

Deleted, with their entry points rewired and their CSS removed: `SettingsModal`,
`ThemeSelectorModal`, `KeybindingsModal`, `SkillsUploadModal`, `styles/settings.css` and
`styles/cheatsheet.css` (already dead). `settings`, `themeSelector` and `cheatsheet` are
gone from `useModalState`, so there are three fewer entries in the Escape-priority list.

Two things worth knowing before touching it:

- **A page route has to be declared in `utils/navigation.ts`.** `PAGE_PATHS` is what stops
  `useUrlRestore` writing `/?session=…` over the path and bouncing the user into a
  conversation. That used to be a one-off `startsWith(KANBAN_PATH)` check; it is a table
  now because this is the second route to hit it.
- **`env` and `headers` values never leave opman.** `ServerView` reports `envNames` and
  `headerNames` only, and `UpsertServer` is a *patch*: an absent field is left alone, and
  secrets are edited by name through `envSet`/`envRemove`. A body that overwrote everything
  it happened to know would delete the credentials it was never shown.

The login endpoints that were missing now exist: `POST /api/mcp/servers/{name}/login`
returns `{ authorizeUrl, redirectUri }`, `POST …/login/finish` takes the URL the browser
landed on, and `POST …/logout` clears the credential. Only the *query* of a pasted callback
is used — host, port and path come from the pending flow — so the finish endpoint cannot be
aimed at anything but opman's own listener.

---

## Loose end: one integration check did not complete

The proxy's protocol behaviour is verified by driving the real binary:

```sh
cargo build
mkdir -p /tmp/proxytest
cat > /tmp/proxytest/mcp.json <<'EOF'
{"servers":{"linear":{"type":"sse","url":"https://mcp.linear.app/sse","auth":"oauth"}}}
EOF
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"linear__authenticate"}}' \
| OPMAN_MCP_CONFIG=/tmp/proxytest/mcp.json ./target/debug/opman mcp-proxy linear
```

Expect: `initialize` answered locally, one synthetic `linear__authenticate` tool, and an
`isError` result naming `opman mcp login linear`. That passes.

What did **not** complete is the same check *through Claude Code*. It returned a generic
"Execution error" I could not attribute to opman's code — the equivalent check for
`opman mcp-skills` through Claude Code passed, and the plumbing is identical. Re-run before
trusting it:

```sh
CFG='{"mcpServers":{"linear":{"command":"'"$PWD"'/target/debug/opman","args":["mcp-proxy","linear"],"env":{"OPMAN_MCP_CONFIG":"/tmp/proxytest/mcp.json"}}}}'
echo "Call the linear__authenticate MCP tool and repeat its message." \
  | claude -p --mcp-config "$CFG" --strict-mcp-config --permission-mode bypassPermissions
```

Note `--mcp-config` and `--allowedTools` are both variadic and will swallow a trailing
positional prompt — pipe the prompt on stdin, as above.

---

## Loose end: an end-to-end OAuth login has never been run

Every piece is unit-tested (RFC 7636 PKCE vector, RFC 9207 ordering, the refresh lock
running its closure exactly once under concurrency), and the settings page now has the
button — but no real authorization server has been talked to. The first real login will
surface whatever a given provider does differently. Worth doing against a
dynamic-registration server (Linear, Sentry).

The page's split flow is what to exercise: it opens `authorizeUrl` in a tab, that tab ends
on an unreachable `http://127.0.0.1:<port>/callback` error page, and the URL from its
address bar is pasted back. `login/finish` delivers the query to the loopback listener the
flow is blocked on, so `callback::validate_response` still does the state and issuer checks.

`opman mcp login` is still not a CLI subcommand — only `mcp-proxy` and `mcp-skills` were
added. It is ~20 lines in `src/cli.rs` + `src/main.rs` and would make a first login
testable without a browser tab at all.

## Verification commands

```sh
cargo test                      # 2736 passing
cargo clippy --all-targets
cd web-ui && npx tsc --noEmit && npx vitest run --config vite.config.ts
wc -l src/mcp_*/**/*.rs src/web/handlers/mcp_*.rs   # nothing over 300
grep -rn "unwrap()\|unsafe" src/mcp_registry src/mcp_oauth src/mcp_proxy src/mcp_skills \
  src/web/handlers/mcp_login.rs src/web/handlers/mcp_upsert.rs \
  | grep -v _tests.rs | grep -v unwrap_or   # must be empty
```

The standing constraint — opman never writes a runner's config file — is worth a CI check
rather than a manual one: hash `~/.claude.json`, `~/.claude/skills`, `~/.codex/config.toml`,
`~/.config/opencode/`, and the project `opencode.json` before a smoke run and re-hash after.

## Background

Design rationale and the measured per-runner MCP timeouts are in the assistant's memory
files (`opman-mcp-registry.md`, `opman-mcp-tool-timeouts.md`, `opman-settings-page.md`), and
the original plan is at `~/.claude/plans/recursive-roaming-shamir.md`.
