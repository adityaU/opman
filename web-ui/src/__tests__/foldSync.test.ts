import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { foldedRanges } from "@codemirror/language";
import { describe, expect, it } from "vitest";
import { nvimFoldMirrorExtension, plainTextFold } from "../code-editor/fold-sync";

describe("Neovim fold mirroring", () => {
  it("provides a delimiter fold for plain-text buffers", () => {
    const state = EditorState.create({
      doc: "function outer() {\n  const value = 42;\n  return value;\n}\n",
      extensions: [nvimFoldMirrorExtension],
    });
    const parent = document.createElement("div");
    document.body.append(parent);
    const view = new EditorView({ state, parent });
    const mode = document.createElement("span");
    mode.className = "cm-nvim-mode-label";
    mode.dataset.mode = "normal";
    view.dom.append(mode);

    expect(plainTextFold(view.state, 0, 19)).toEqual({ from: 18, to: 55 });
    view.contentDOM.dispatchEvent(new KeyboardEvent("keydown", { key: "z", bubbles: true }));
    view.contentDOM.dispatchEvent(new KeyboardEvent("keydown", { key: "c", bubbles: true }));
    expect(foldedRanges(view.state).size).toBe(1);
    view.destroy();
    parent.remove();
  });
});
