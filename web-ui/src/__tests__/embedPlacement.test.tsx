/**
 * An embed's request to open something reaches the parent's planners, tagged
 * with the embed's server; foreign origins, junk and repeats do not.
 */
import { describe, it, expect, vi } from "vitest";
import { renderHook } from "@testing-library/react";

const runCommand = vi.fn();
vi.mock("../keybindings/KeymapContext", () => ({ useKeymapContext: () => ({ runCommand }) }));

const { useEmbedRequests } = await import("../workspace/useEmbedRequests");
import { readFromEmbed, readFromParent } from "../embed/messages";
import { planForeignOpen, withServer } from "../workspace/foreignOpen";
import { planFileOpen } from "../workspace/fileOpen";
import { EMPTY_HISTORY } from "../workspace/history";
import { asPaneId, type PaneNode, type WidgetState } from "../workspace/types";

const leaf = (id: string, widget: WidgetState | null): PaneNode => ({
  type: "leaf", id: asPaneId(id), widget, history: EMPTY_HISTORY,
});
const file = (path: string): WidgetState => ({
  kind: "files", projectPath: "/r", sessionId: "embed", open: { path, line: 3, seq: 0 },
});

function post(data: unknown, origin = location.origin) {
  window.dispatchEvent(new MessageEvent("message", { data, origin, source: null }));
}

describe("message validation", () => {
  it("accepts well-formed messages and strips any server off the widget", () => {
    const message = readFromEmbed({ type: "opman:open-widget", server: "dev", widget: { ...file("/r/a"), server: "x" } });
    expect(message).toEqual({ type: "opman:open-widget", server: "dev", widget: file("/r/a") });
    expect(readFromParent({ type: "opman:set-widget", widget: file("/r/a") })?.widget).toEqual(file("/r/a"));
  });

  it("rejects junk, missing servers and non-workspace commands", () => {
    expect(readFromEmbed(null)).toBeNull();
    expect(readFromEmbed({ type: "opman:open-widget", widget: file("/r/a") })).toBeNull();
    expect(readFromEmbed({ type: "opman:open-widget", server: "dev", widget: { kind: "x" } })).toBeNull();
    expect(readFromEmbed({ type: "opman:run-command", server: "dev", command: "app.quit" })).toBeNull();
  });

  it("keeps a live browser reveal", () => {
    const widget = { kind: "browser", projectPath: "/r", browserId: "proj:r", url: "https://a", reveal: 7 };
    expect(readFromParent({ type: "opman:set-widget", widget })?.widget).toMatchObject({ reveal: 7 });
  });
});

describe("foreign placement", () => {
  it("tags the server, and untags the own one", () => {
    expect(withServer(file("/r/a"), "dev", "home").server).toBe("dev");
    expect("server" in withServer({ ...file("/r/a"), server: "dev" }, "home", "home")).toBe(false);
  });

  it("reuses a files pane only on the same server", () => {
    const local = leaf("p1", { kind: "files", projectPath: "/r", sessionId: "p1", open: null });
    const remote = leaf("p2", { kind: "files", projectPath: "/r", sessionId: "p2", open: null, server: "dev" });
    const plan = planForeignOpen(withServer(file("/r/a"), "dev", "home"), [local, remote], local.id, 9);
    expect(plan).toMatchObject({ action: "place", pane: "p2", widget: { server: "dev", open: { path: "/r/a", seq: 9 } } });

    const ownPlan = planFileOpen({ path: "/r/b", line: null, seq: 1 }, [remote, local], remote.id, [{ path: "/r", name: "r" }]);
    expect(ownPlan).toMatchObject({ action: "place", pane: "p1" });
  });

  it("splits beside the focused pane when nothing matches", () => {
    const chat = leaf("p1", { kind: "chat", projectPath: "/r", sessionId: "s", engine: null });
    const plan = planForeignOpen({ kind: "git", projectPath: "/r", server: "dev" }, [chat], chat.id, 1);
    expect(plan).toEqual({ action: "split", pane: "p1", widget: { kind: "git", projectPath: "/r", server: "dev" } });
  });
});

describe("useEmbedRequests", () => {
  it("places same-origin open requests once, tagged with the sender's server", () => {
    const open = vi.fn();
    const { unmount } = renderHook(() => useEmbedRequests(true, open));
    post({ type: "opman:open-widget", server: "dev", widget: file("/r/a") });
    post({ type: "opman:open-widget", server: "dev", widget: { ...file("/r/a"), open: { path: "/r/a", line: 3, seq: 5 } } });
    post({ type: "opman:open-widget", server: "dev", widget: file("/r/b") }, "https://evil.example");
    post({ type: "opman:run-command", server: "dev", command: "workspace.splitRight" });
    expect(open).toHaveBeenCalledTimes(1);
    expect(open.mock.calls[0][0]).toMatchObject({ kind: "files", server: "dev", open: { path: "/r/a" } });
    expect(runCommand).toHaveBeenCalledWith("workspace.splitRight");
    unmount();
  });

  it("listens to nothing inside an embed", () => {
    const open = vi.fn();
    renderHook(() => useEmbedRequests(false, open));
    post({ type: "opman:open-widget", server: "dev", widget: file("/r/c") });
    expect(open).not.toHaveBeenCalled();
  });
});
