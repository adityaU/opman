import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { foldable } from "@codemirror/language";
import { plainTextFold } from "../code-editor/fold";
import { foldGutterExtension } from "../code-editor/fold";

const BLOCK = "function outer() {\n  const value = 42;\n  return value;\n}\n";

function stateOf(doc: string) {
  return EditorState.create({ doc, extensions: [foldGutterExtension] });
}

describe("plainTextFold", () => {
  it("folds a delimiter block in a buffer with no language", () => {
    const state = stateOf(BLOCK);
    const line = state.doc.line(1);
    const range = plainTextFold(state, line.from, line.to);
    expect(range).not.toBeNull();
    expect(state.doc.sliceString(range?.from ?? 0, range?.to ?? 0))
      .toBe("\n  const value = 42;\n  return value;\n");
  });

  it("is reachable through CodeMirror's own foldable()", () => {
    const state = stateOf(BLOCK);
    const line = state.doc.line(1);
    expect(foldable(state, line.from, line.to)).not.toBeNull();
  });

  it("returns null for a line that opens nothing", () => {
    const state = stateOf(BLOCK);
    const line = state.doc.line(2);
    expect(plainTextFold(state, line.from, line.to)).toBeNull();
  });

  it("returns null when the block never closes", () => {
    const state = stateOf("function outer() {\n  const value = 42;\n");
    const line = state.doc.line(1);
    expect(plainTextFold(state, line.from, line.to)).toBeNull();
  });
});
