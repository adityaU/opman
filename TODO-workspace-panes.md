# TODO — desktop workspace (panes/windows/widgets)

Everything the previous handoff listed is done and verified. What is left is a
short list of genuinely new observations, one of which is not this feature's
fault.

- Design + architecture: `docs/workspace-panes-plan.md`
- Design record: `docs/workspace-panes-design.md`
- Code: `web-ui/src/workspace/`, `web-ui/src/hooks/sse/sessionStore.ts`,
  `src/web/pty_manager/`
- Nothing is committed. `git status` shows a large dirty tree on `develop`.

---

## How to run and verify

**Never restart the main opman instance** — it hosts the agent session doing the
work. Start a second one instead:

```bash
cd web-ui && npx vite build && cd .. && cargo build --release
./target/release/opman --web-only --web-port 7799 --web-user t --web-pass t
node ~/opencode-tools/opman/verify-workspace.mjs
```

That script is the harness. It drives the second instance with the Playwright
already in `web-ui/node_modules` and checks mobile at 768px, PTY scrollback
replay, terminal busy state, inline window rename, widget drag-and-drop, and all
four appearance combinations. Screenshots land in `/tmp/wsp-shots`.

**Do not wait on `networkidle`.** The app holds SSE streams open, so it never
fires — this cost two debugging rounds. Use `domcontentloaded` plus a selector.

Tests: `cargo test` is **2758 passing, 0 failing**. `cd web-ui && npx vitest run`
is **545 passing, 4 failing** — the four are pre-existing token/URL failures in
`src/__tests__/api.test.ts` and are unrelated to this work. `npx tsc --noEmit`
must be clean, and `src/__tests__/keybindingsMatrix.test.ts` must stay at 43/43.

---

## Done since the last handoff

1. **PTY scrollback replay.** `buffer.rs` keeps a 128 KiB retained window that
   compaction never discards; `snapshot()` returns it and seeks the reader to
   the tip. `terminal_stream` leads with it on `replay=1`, which
   `useTerminalLifecycle` asks for only when `attachOrSpawn` actually attached.
   Verified: `echo MARKER_42`, reload, marker still on screen, same PTY id.
2. **Mobile at 768px.** Confirmed: `.wsp-root` absent, `.chat-main` present,
   mobile dock renders.
3. **Flat and light appearances.** All four combinations screenshotted. Toggling
   the class alone does *not* repaint — the palette variables are set by JS at
   mount, so the harness writes `opman-theme-mode` / `opman-appearance` and
   reloads.
4. **Per-pane engine selection.** `PaneEngine` on the chat widget, persisted.
   Verified two panes on OpenCode and Codex simultaneously, surviving a reload.
5. **Inline window rename.** Portalled field anchored to the rail chip;
   `window.prompt()` is gone.
6. **Busy state for terminal panes.** Foreground process group via
   `tcgetpgrp`, polled from `GET /api/pty/activity`.
7. **Drag a widget between panes.** Pane header kind chip is the grab handle;
   the target overlay gained a drop mode; `swapWidgets` swaps widgets, not panes.
8. **ChatWidget parity.** Subagent transcripts, search highlighting, bookmarks,
   older-message pagination (per session, via `loadOlderIn`), and per-pane
   permission/question docks.

---

## What is left

**A light-theme defect in the git panel, not the workspace.** In light mode the
staged-file row renders as a dark bar with near-invisible text
(`/tmp/wsp-shots/22-flat-light.png`, "STAGED (1)"). Pre-existing — no
workspace change touches git-panel CSS — but it is now easy to see because the
workspace puts git side by side with everything else.

**Window names longer than ~4 characters truncate on the rail chip.** The chip
is 30px wide with a 26px name; the full name only shows in the tooltip and the
window switcher. The rename field allows 24 characters, so the mismatch is
reachable. Either shorten the field's limit or let the chip widen on hover.

**Search is per pane but shares one open/close flag.** `mod+f` opens the find
bar in the focused chat pane, which is the right scoping, but two panes cannot
have a search open at once. Nobody has asked for that; noting it so the next
person does not read it as a bug.

**`impeccable` finish protocol.** `detect.mjs` reports `[]` and the token audit
is clean under `workspace-*`. `docs/workspace-panes-design.md` is the
documenter's output, written by hand. The skill's own routing wanted a
`PRODUCT.md` first (`concept-seed.mjs` refuses to run without one), which was
never written and never became blocking.

---

## Gotchas worth knowing before you edit

- **The pane tree is n-ary.** Splitting right three times must produce one split
  with four children, not three nested levels. `splitPane` appends to the parent
  when the direction matches. Tests in `__tests__/workspaceTree.test.ts` pin it.
- **`swapPanes` and `swapWidgets` must each be a single traversal.** Two
  sequential `mapPane` calls leave the tree briefly holding two nodes with the
  same id and the second pass rewrites both. There are tests for both.
- **`swapWidgets`, not `swapPanes`, for drag.** A pane's id is its focus scope
  and every `data-pane-id` lookup's key; moving it with the widget would make
  pane 1 land on the right.
- **`sessionStore` opens no connections and owns no fetching.** `useSSE` stays
  the single reader of the event stream and pushes snapshots in. Keep it that
  way — two sources of truth for a live transcript is a bug factory. Its
  `publishSession` identity check must cover every field of `SessionView`, or a
  pane goes stale on the field you forgot.
- **Terminal busy is polled, not streamed.** One endpoint for every PTY, because
  a pane in a background window is unmounted and has no output stream. Do not
  add a second signal on the stream.
- **A fresh PTY must not replay.** `attachOrSpawn` returns whether it attached;
  passing `replay` unconditionally would repaint another tab's history into a
  new one.
- **`mod+k <letter>` is crowded.** `q`, `u`, `r`, `w`, `x` are all taken by
  `when`-scoped commands. The conflict test *permits* the overlap because the
  clauses differ, but the matcher still picks one, so a shadowed chord looks
  broken rather than failing loudly. Check `grep -rn 'key: "mod+k' layers/`
  before claiming one.
- **Overlay keys are commands**, gated on `workspaceTargeting` /
  `workspaceOpener`, not ad-hoc listeners. A pointer drag deliberately does not
  arm them — keep it that way.
- Opener draft `sessionId` is a real tri-state: `undefined` unasked, `null` =
  "new session here", string = existing. Collapsing the first two makes a chat
  draft look finished before anything is chosen.
- **Pane engine `null` is meaningful**: it is "follow the shell", not "no
  engine". Materialise from `defaultEngine` on the first change rather than
  writing a partial one.
