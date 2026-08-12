import { beforeEach, describe, expect, it } from "vitest";
import {
  loadEditorEngine,
  parseEditorEngine,
  saveEditorEngine,
  STORAGE_KEY,
  VERSION,
} from "../editor-engine/preference";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => [...values.keys()][index] ?? null,
    get length() { return values.size; },
  };
}

describe("editor engine preference", () => {
  let storage: Storage;

  beforeEach(() => {
    storage = memoryStorage();
  });

  it("seeds Vim users with Neovim and persists the seed", () => {
    expect(loadEditorEngine(storage, "vim")).toBe("neovim");
    expect(storage.getItem(STORAGE_KEY)).toBe(
      JSON.stringify({ version: VERSION, engine: "neovim" }),
    );
  });

  it("seeds normal users with CodeMirror and persists the seed", () => {
    expect(loadEditorEngine(storage, "normal")).toBe("codemirror");
    expect(JSON.parse(storage.getItem(STORAGE_KEY) ?? "null")).toEqual({
      version: VERSION,
      engine: "codemirror",
    });
  });

  it("lets a stored choice win over the keymap seed", () => {
    saveEditorEngine("codemirror", storage);
    expect(loadEditorEngine(storage, "vim")).toBe("codemirror");
  });

  it("repairs corrupt storage without throwing", () => {
    storage.setItem(STORAGE_KEY, "not json");
    expect(loadEditorEngine(storage, "vim")).toBe("codemirror");
    expect(JSON.parse(storage.getItem(STORAGE_KEY) ?? "null")).toEqual({
      version: VERSION,
      engine: "codemirror",
    });
    expect(parseEditorEngine({ engine: "other" })).toBe("codemirror");
  });

  it("round-trips a saved Neovim choice", () => {
    saveEditorEngine("neovim", storage);
    expect(loadEditorEngine(storage, "normal")).toBe("neovim");
  });
});
