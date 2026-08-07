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

describe("step sequence", () => {
  it("asks chat for a session and the others for nothing more", () => {
    expect(stepsFor("chat")).toEqual(["kind", "project", "session"]);
    expect(stepsFor("git")).toEqual(["kind", "project"]);
    expect(stepsFor("terminal")).toEqual(["kind", "project"]);
    expect(stepsFor(null)).toEqual(["kind"]);
  });

  it("walks kind → project → session", () => {
    expect(currentStep(EMPTY_DRAFT)).toBe("kind");
    expect(currentStep(advance(EMPTY_DRAFT, "chat"))).toBe("project");
    expect(currentStep(chatDraft)).toBe("session");
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
});

describe("retreating", () => {
  it("clears the step it goes back to", () => {
    expect(retreat(chatDraft).projectPath).toBeNull();
    expect(retreat(advance(EMPTY_DRAFT, "chat")).kind).toBeNull();
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
    const widget = toWidget(advance(advance(backToKind, "terminal"), "/repo"));
    expect(widget).toEqual({ kind: "terminal", projectPath: "/repo", ptyIds: [] });
    expect(widget).not.toHaveProperty("sessionId");
  });
});
