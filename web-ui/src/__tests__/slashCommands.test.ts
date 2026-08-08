import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  COMMANDS,
  OPMAN_SLASH_COMMANDS,
  RUNNER_SLASH_COMMANDS,
  findSlashCommand,
} from "../keybindings/commands";
import { takesArguments } from "../prompt-input/helpers";
import { createHandleCommand } from "../chatLayoutHandlers";
import type { HandlerDeps } from "../chatLayoutHandlers";

vi.mock("../api", () => ({
  executeCommand: vi.fn(async () => ({ ok: true })),
  sendMessage: vi.fn(),
  abortSession: vi.fn(),
  replyPermission: vi.fn(),
  replyQuestion: vi.fn(),
  rejectQuestion: vi.fn(),
  newSession: vi.fn(),
  switchProject: vi.fn(),
  fetchAppState: vi.fn(),
}));

import { executeCommand } from "../api";

describe("the slash registry", () => {
  it("gives every slash a unique name", () => {
    const names = [...OPMAN_SLASH_COMMANDS, ...RUNNER_SLASH_COMMANDS].map((c) => c.slash.name);
    expect(names.length).toBe(new Set(names).size);
  });

  it("resolves only opman's own names", () => {
    // opman's, because opman implements it...
    expect(findSlashCommand("todos")?.id).toBe("chat.todoPanel");
    // ...and not the runner's, because forwarding is what makes an unknown name work.
    expect(findSlashCommand("compact")).toBeUndefined();
    expect(findSlashCommand("some-agent-skill")).toBeUndefined();
  });

  it("keeps no catalog of agent commands", () => {
    // The few runner-slash entries exist only so a *chord* can reach them. Anything
    // beyond that would be opman guessing at an agent's vocabulary.
    const forwarded = RUNNER_SLASH_COMMANDS.map((c) => c.slash.name).sort();
    expect(forwarded).toEqual(["clear", "compact", "fork", "redo", "share", "undo"]);
  });

  it("classifies every slash as exactly one of opman's or the runner's", () => {
    const slashed = COMMANDS.filter((c) => c.slash);
    expect(slashed.length).toBe(OPMAN_SLASH_COMMANDS.length + RUNNER_SLASH_COMMANDS.length);
  });
});

describe("argument hints", () => {
  it("reads the runner's own hint rather than a list of names", () => {
    expect(takesArguments({ name: "grep", args: "<pattern>" })).toBe(true);
    expect(takesArguments({ name: "review", template: "Review $ARGUMENTS please" })).toBe(true);
    expect(takesArguments({ name: "commit", template: "Commit the staged changes" })).toBe(false);
    // A command opman has never seen, with nothing said about it, runs as-is.
    expect(takesArguments({ name: "brand-new-skill" })).toBe(false);
  });
});

describe("dispatching a typed slash", () => {
  const deps = (overrides: Partial<HandlerDeps> = {}) =>
    ({
      activeSessionId: "ses_1",
      runCommandId: vi.fn(() => true),
      refreshState: vi.fn(),
      addToast: vi.fn(),
      ...overrides,
    }) as unknown as HandlerDeps;

  beforeEach(() => {
    vi.mocked(executeCommand).mockClear();
  });

  it("runs an opman command through the command registry", async () => {
    const d = deps();
    await createHandleCommand(d)("todos");

    expect(d.runCommandId).toHaveBeenCalledWith("chat.todoPanel");
    expect(executeCommand).not.toHaveBeenCalled();
  });

  it("forwards everything else to the runner, arguments included", async () => {
    const d = deps();
    await createHandleCommand(d)("some-agent-skill", "with args");

    expect(d.runCommandId).not.toHaveBeenCalled();
    expect(executeCommand).toHaveBeenCalledWith("ses_1", "some-agent-skill", "with args");
  });

  it("forwards a runner-slash command rather than looping back into its own handler", async () => {
    // `chat.compact`'s handler *is* "send /compact", so resolving the name locally would
    // call the handler that sends the name that resolves locally, forever.
    const d = deps();
    await createHandleCommand(d)("compact");

    expect(d.runCommandId).not.toHaveBeenCalled();
    expect(executeCommand).toHaveBeenCalledWith("ses_1", "compact", undefined);
  });

  it("does not forward with no session to forward to", async () => {
    const d = deps({ activeSessionId: null });
    await createHandleCommand(d)("some-agent-skill");

    expect(executeCommand).not.toHaveBeenCalled();
  });

  it("reports a rejected command instead of swallowing it", async () => {
    vi.mocked(executeCommand).mockRejectedValueOnce(new Error("no such command"));
    const d = deps();
    await createHandleCommand(d)("nope");

    expect(d.addToast).toHaveBeenCalledWith("no such command", "error");
  });
});
