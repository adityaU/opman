import { describe, expect, it } from "vitest";
import { createImeInput } from "../input/ime";

describe("Neovim IME input", () => {
  it("positions a contenteditable and pastes only committed composition data", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const sent: string[] = [];
    const ime = createImeInput({ container, send: (message) => sent.push(message.type === "paste" ? message.data : "") });
    ime.updateCursor({ left: 12, top: 24, width: 8, height: 16 });
    expect(ime.element.style.left).toBe("12px");
    ime.element.dispatchEvent(new CompositionEvent("compositionstart"));
    ime.element.textContent = "にほん";
    ime.element.dispatchEvent(new CompositionEvent("compositionupdate", { data: "にほん" }));
    ime.element.dispatchEvent(new CompositionEvent("compositionend", { data: "日本" }));
    expect(sent).toEqual(["日本"]);
    ime.destroy();
    expect(container.contains(ime.element)).toBe(false);
  });

  it("forwards ordinary committed input and clamps the hidden cursor box", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const sent: string[] = [];
    const ime = createImeInput({ container, send: (message) => {
      if (message.type === "paste") sent.push(message.data);
    } });

    ime.updateCursor({ left: 3, top: 4, width: 0, height: -2 });
    ime.element.textContent = "é";
    ime.element.dispatchEvent(new Event("input"));
    expect(sent).toEqual(["é"]);
    expect(ime.element.style.width).toBe("1px");
    expect(ime.element.style.height).toBe("1px");
    ime.destroy();
  });
});
