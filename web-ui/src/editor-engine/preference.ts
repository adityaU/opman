import { useCallback, useEffect, useState } from "react";
import type { Mode } from "../keybindings/types";

export type EditorEngine = "codemirror" | "neovim";

export const STORAGE_KEY = "opman.editor-engine";
export const VERSION = 1;
export const EDITOR_ENGINE_CHANGED = "opman:editor-engine-changed";

const SAFE_DEFAULT: EditorEngine = "codemirror";

type Reader = Pick<Storage, "getItem">;
type Writer = Pick<Storage, "setItem">;
type StorageAdapter = Reader & Writer;

interface Envelope {
  readonly version: number;
  readonly engine: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function seedFromKeymap(mode: Mode): EditorEngine {
  return mode === "vim" ? "neovim" : SAFE_DEFAULT;
}

/** Total parser for the stored engine value. Unknown values repair to CodeMirror. */
export function parseEditorEngine(value: unknown): EditorEngine {
  return value === "neovim" || value === "codemirror" ? value : SAFE_DEFAULT;
}

function readEnvelope(storage: Reader): Envelope | null {
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed) || typeof parsed.version !== "number" || !("engine" in parsed)) {
      return null;
    }
    return { version: parsed.version, engine: parsed.engine };
  } catch {
    return null;
  }
}

function hasStoredPreference(storage: Reader): boolean {
  try {
    return storage.getItem(STORAGE_KEY) !== null;
  } catch {
    return false;
  }
}

/** Load the per-device choice, seeding first use from the current keymap mode. */
export function loadEditorEngine(
  storage: StorageAdapter = localStorage,
  keymapMode: Mode = "normal",
): EditorEngine {
  const envelope = readEnvelope(storage);
  if (!envelope && !hasStoredPreference(storage)) {
    const seeded = seedFromKeymap(keymapMode);
    saveEditorEngine(seeded, storage);
    return seeded;
  }

  if (!envelope || envelope.version !== VERSION) {
    saveEditorEngine(SAFE_DEFAULT, storage);
    return SAFE_DEFAULT;
  }

  const parsed = parseEditorEngine(envelope.engine);
  if (parsed !== envelope.engine) saveEditorEngine(parsed, storage);
  return parsed;
}

export function saveEditorEngine(
  engine: EditorEngine,
  storage: Writer = localStorage,
): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify({ version: VERSION, engine }));
  } catch {
    // A blocked or full localStorage still leaves the choice usable for this render.
  }
}

/** React bridge for live preference changes across mounted editor panes. */
export function useEditorEngine(keymapMode: Mode): readonly [EditorEngine, (engine: EditorEngine) => void] {
  const [engine, setEngine] = useState<EditorEngine>(() => loadEditorEngine(localStorage, keymapMode));

  useEffect(() => {
    const onChange = () => setEngine(loadEditorEngine(localStorage, keymapMode));
    window.addEventListener(EDITOR_ENGINE_CHANGED, onChange);
    window.addEventListener("storage", onChange);
    return () => {
      window.removeEventListener(EDITOR_ENGINE_CHANGED, onChange);
      window.removeEventListener("storage", onChange);
    };
  }, [keymapMode]);

  const setPreference = useCallback((next: EditorEngine) => {
    saveEditorEngine(next);
    setEngine(next);
    window.dispatchEvent(new Event(EDITOR_ENGINE_CHANGED));
  }, []);

  return [engine, setPreference] as const;
}
