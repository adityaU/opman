import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api/pty", () => ({
  spawnPty: vi.fn(async () => ({ id: "x", ok: true })),
  ptyWrite: vi.fn(async () => undefined),
}));

import { ptyWrite, spawnPty } from "../api/pty";
import { startLogin } from "../settings-page/acp/startLogin";
import { authStatusOf } from "../settings-page/acp/status";
import { queueWidgetOpen, takeQueuedWidgets } from "../workspace/pendingOpen";

describe("authStatusOf", () => {
  it("says Checking while the agent is being asked", () => {
    const status = authStatusOf({ status: "checking" });
    expect(status).toMatchObject({ label: "Checking…", checking: true, connected: false });
  });

  it("says Connected for a logged-in agent and offers no login", () => {
    const status = authStatusOf({
      status: "known",
      auth: { state: "connected", detail: "", methods: ["Google"], loginCommand: null },
    });
    expect(status).toMatchObject({ label: "Connected", tone: "success", connected: true, command: null });
  });

  it("says Not connected with the login command when a login is possible", () => {
    const status = authStatusOf({
      status: "known",
      auth: { state: "not_connected", detail: "Log in.", methods: ["Google"], loginCommand: "opman acp-login a" },
    });
    expect(status).toMatchObject({ label: "Not connected", tone: "warning", command: "opman acp-login a" });
  });

  it("keeps the reason when asking failed, and offers no login", () => {
    const status = authStatusOf({ status: "failed", message: "boom" });
    expect(status).toMatchObject({ label: "Not connected", detail: "boom", command: null });
  });
});

describe("pending workspace opens", () => {
  it("hands over queued widgets once and then is empty", () => {
    queueWidgetOpen({ kind: "terminal", projectPath: "/p", ptyId: "a" });
    queueWidgetOpen({ kind: "terminal", projectPath: "/p", ptyId: "b" });
    expect(takeQueuedWidgets().map((w) => (w.kind === "terminal" ? w.ptyId : ""))).toEqual(["a", "b"]);
    expect(takeQueuedWidgets()).toEqual([]);
  });
});

describe("startLogin", () => {
  beforeEach(() => {
    takeQueuedWidgets();
    vi.clearAllMocks();
  });

  it("starts a shell in the project, types the command, and queues a terminal on it", async () => {
    await startLogin({ id: "antigravity", displayName: "Google Antigravity" }, "'/bin/opman' acp-login antigravity", "/proj");

    const [kind, id, , , options] = vi.mocked(spawnPty).mock.calls[0];
    expect(kind).toBe("shell");
    expect(options).toMatchObject({ project: "/proj", label: "Login: Google Antigravity" });
    expect(ptyWrite).toHaveBeenCalledWith(id, "'/bin/opman' acp-login antigravity\n");
    expect(takeQueuedWidgets()).toEqual([{ kind: "terminal", projectPath: "/proj", ptyId: id }]);
  });

  it("queues nothing when the shell cannot start", async () => {
    vi.mocked(spawnPty).mockRejectedValueOnce(new Error("no pty"));
    await expect(startLogin({ id: "a", displayName: "A" }, "cmd", "/p")).rejects.toThrow("no pty");
    expect(takeQueuedWidgets()).toEqual([]);
  });
});
