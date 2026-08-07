import { describe, expect, it } from "vitest";
import { evaluateWhen, whenEvaluator } from "../keybindings/when";

const ctx = {
  focus: "editor",
  sessionActive: true,
  editorDirty: false,
  hasQueue: undefined,
  empty: "",
};

const evaluate = (clause: string) => evaluateWhen(clause, ctx);

describe("evaluateWhen", () => {
  it("treats an absent clause as always true", () => {
    expect(evaluateWhen(undefined, ctx)).toBe(true);
    expect(evaluateWhen("", ctx)).toBe(true);
  });

  it("reads a bare identifier as truthiness", () => {
    expect(evaluate("sessionActive")).toBe(true);
    expect(evaluate("editorDirty")).toBe(false);
    expect(evaluate("hasQueue")).toBe(false);
    expect(evaluate("empty")).toBe(false);
    expect(evaluate("neverDefined")).toBe(false);
  });

  it("compares with == and !=", () => {
    expect(evaluate("focus==editor")).toBe(true);
    expect(evaluate("focus==terminal")).toBe(false);
    expect(evaluate("focus!=terminal")).toBe(true);
  });

  it("negates", () => {
    expect(evaluate("!editorDirty")).toBe(true);
    expect(evaluate("!sessionActive")).toBe(false);
    expect(evaluate("!!sessionActive")).toBe(true);
  });

  it("combines with && and ||", () => {
    expect(evaluate("focus==editor && sessionActive")).toBe(true);
    expect(evaluate("focus==editor && editorDirty")).toBe(false);
    expect(evaluate("editorDirty || sessionActive")).toBe(true);
    expect(evaluate("editorDirty || hasQueue")).toBe(false);
  });

  it("gives && higher precedence than ||", () => {
    expect(evaluateWhen("a || b && c", { a: true, b: true, c: false })).toBe(true);
    expect(evaluateWhen("a && b || c", { a: false, b: true, c: true })).toBe(true);
  });

  it("honours parentheses", () => {
    expect(evaluateWhen("(a || b) && c", { a: true, b: false, c: false })).toBe(false);
    expect(evaluateWhen("!(a && b)", { a: true, b: false })).toBe(true);
  });

  it("accepts dots, dashes and slashes in identifiers", () => {
    expect(evaluateWhen("focus==code-editor", { focus: "code-editor" })).toBe(true);
    expect(evaluateWhen("view==git/log", { view: "git/log" })).toBe(true);
  });

  it("returns false for a malformed clause instead of throwing", () => {
    expect(evaluate("focus ==")).toBe(false);
    expect(evaluate("&& focus")).toBe(false);
    expect(evaluate("(focus==editor")).toBe(false);
    expect(evaluate("focus editor")).toBe(false);
    expect(evaluate("focus==editor @@@")).toBe(false);
  });

  it("evaluates every real clause used by the keymap", () => {
    const clauses = [
      "sessionActive",
      "sessionBusy",
      "focus==explorer",
      "focus==git",
      "editorOpen",
      "editorDirty",
      "anyDirty",
      "composerFocused",
      "permissionPending",
      "taskHasSession",
      "diffReviewOpen",
      "explorerFinderActive",
    ];
    for (const clause of clauses) {
      expect(() => evaluateWhen(clause, ctx)).not.toThrow();
    }
  });
});

describe("whenEvaluator", () => {
  it("caches per clause within one snapshot", () => {
    let reads = 0;
    const evaluator = whenEvaluator(
      new Proxy({ a: true } as Record<string, boolean>, {
        get(target, key: string) {
          reads += 1;
          return target[key];
        },
      }),
    );

    expect(evaluator("a")).toBe(true);
    expect(evaluator("a")).toBe(true);
    expect(reads).toBe(1);
  });
});
