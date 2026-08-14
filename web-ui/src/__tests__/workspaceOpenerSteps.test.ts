import { describe, expect, it } from "vitest";
import {
  advance,
  currentStep,
  EMPTY_DRAFT,
  isComplete,
  retreat,
  stepsFor,
  toWidget,
} from "../workspace/opener/steps";

const chatDraft = advance(advance(EMPTY_DRAFT, "chat"), "/repo");
const gitDraft = advance(advance(EMPTY_DRAFT, "git"), "/repo");
const terminalDraft = advance(advance(EMPTY_DRAFT, "terminal"), "/repo");

describe("step sequence", () => {
  it("asks each kind for the thing it is of, and the rest for nothing more", () => {
    expect(stepsFor("chat")).toEqual(["kind", "project", "session"]);
    expect(stepsFor("terminal")).toEqual(["kind", "project", "shell"]);
    expect(stepsFor("git")).toEqual(["kind", "project"]);
    expect(stepsFor("files")).toEqual(["kind", "project"]);
    expect(stepsFor(null)).toEqual(["kind"]);
  });

  it("walks kind → project → session", () => {
    expect(currentStep(EMPTY_DRAFT)).toBe("kind");
    expect(currentStep(advance(EMPTY_DRAFT, "chat"))).toBe("project");
    expect(currentStep(chatDraft)).toBe("session");
  });

  it("walks kind → project → shell for a terminal", () => {
    expect(currentStep(advance(EMPTY_DRAFT, "terminal"))).toBe("project");
    expect(currentStep(terminalDraft)).toBe("shell");
    expect(currentStep(advance(terminalDraft, "pty-a"))).toBeNull();
  });
});

describe("advancing", () => {
  it("rejects a kind that is not a widget", () => {
    expect(advance(EMPTY_DRAFT, "hologram")).toBe(EMPTY_DRAFT);
    expect(advance(EMPTY_DRAFT, null)).toBe(EMPTY_DRAFT);
  });

  it("treats a null session as 'new session here', not as no answer", () => {
    const done = advance(chatDraft, null);
    expect(done.sessionId).toBeNull();
    expect(toWidget(done)).toEqual({ kind: "chat", projectPath: "/repo", sessionId: null, engine: null });
  });

  it("treats a null shell as 'a new shell', not as no answer", () => {
    const done = advance(terminalDraft, null);
    expect(done.ptyId).toBeNull();
    expect(toWidget(done)).toEqual({ kind: "terminal", projectPath: "/repo", ptyId: null });
  });

  it("carries the chosen shell onto the widget", () => {
    expect(toWidget(advance(terminalDraft, "pty-a"))).toEqual({
      kind: "terminal",
      projectPath: "/repo",
      ptyId: "pty-a",
    });
  });
});

describe("completeness", () => {
  it("is complete for a non-chat widget as soon as it has a project", () => {
    expect(isComplete(gitDraft)).toBe(true);
    expect(isComplete(advance(EMPTY_DRAFT, "git"))).toBe(false);
  });

  it("holds chat open until the session step is answered, not merely reached", () => {
    expect(isComplete(advance(EMPTY_DRAFT, "chat"))).toBe(false);
    expect(isComplete(chatDraft)).toBe(false);
    expect(isComplete(advance(chatDraft, "s1"))).toBe(true);
    // "new session here" is an answer, so it completes too.
    expect(isComplete(advance(chatDraft, null))).toBe(true);
  });

  it("holds a terminal open until the shell step is answered", () => {
    expect(isComplete(terminalDraft)).toBe(false);
    expect(isComplete(advance(terminalDraft, "pty-a"))).toBe(true);
    expect(isComplete(advance(terminalDraft, null))).toBe(true);
  });
});

describe("retreating", () => {
  it("clears the step it goes back to", () => {
    expect(retreat(chatDraft).projectPath).toBeNull();
    expect(retreat(advance(EMPTY_DRAFT, "chat")).kind).toBeNull();
  });

  it("unanswers the shell step before the project", () => {
    const answered = advance(terminalDraft, "pty-a");
    const back = retreat(answered);
    expect(back.ptyId).toBeUndefined();
    expect(back.projectPath).toBe("/repo");
    expect(retreat(back).projectPath).toBeNull();
  });

  it("stops at the first step rather than going negative", () => {
    expect(retreat(EMPTY_DRAFT)).toBe(EMPTY_DRAFT);
  });
});

describe("toWidget", () => {
  it("refuses an unfinished draft", () => {
    expect(toWidget(EMPTY_DRAFT)).toBeNull();
    expect(toWidget(advance(EMPTY_DRAFT, "git"))).toBeNull();
  });

  /** The union is what stops a terminal from carrying a session id. */
  it("drops a session id left over from an abandoned chat branch", () => {
    const withSession = advance(chatDraft, "s1");
    // session → project → kind: three steps back to an empty draft.
    const backToKind = retreat(retreat(retreat(withSession)));
    const restarted = advance(advance(backToKind, "terminal"), "/repo");
    const widget = toWidget(advance(restarted, null));
    expect(widget).toEqual({ kind: "terminal", projectPath: "/repo", ptyId: null });
    expect(widget).not.toHaveProperty("sessionId");
  });
});
