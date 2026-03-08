/**
 * Unit tests for ToolCall pure helper functions:
 * - formatToolName
 * - formatDuration
 * - parseOutput
 * - guessLanguage
 */
import { describe, it, expect } from "vitest";
import {
  formatToolName,
  formatDuration,
  parseOutput,
  guessLanguage,
} from "../ToolCall";

// ── formatToolName ──────────────────────────────────────
describe("formatToolName", () => {
  it("removes duplicate prefix when first two parts match (3+ parts)", () => {
    expect(formatToolName("provider_provider_action")).toBe("action");
  });

  it("removes duplicate prefix with multi-word suffix", () => {
    expect(formatToolName("foo_foo_bar_baz")).toBe("bar_baz");
  });

  it("deduplicates when exactly two identical parts", () => {
    expect(formatToolName("bash_bash")).toBe("bash");
  });

  it("returns original when parts do not duplicate", () => {
    expect(formatToolName("read_file")).toBe("read_file");
  });

  it("returns original single-word name", () => {
    expect(formatToolName("task")).toBe("task");
  });

  it("returns original when three parts but first two differ", () => {
    expect(formatToolName("a_b_c")).toBe("a_b_c");
  });

  it("handles empty string", () => {
    expect(formatToolName("")).toBe("");
  });
});

// ── formatDuration ──────────────────────────────────────
describe("formatDuration", () => {
  it("returns milliseconds for < 1000ms", () => {
    expect(formatDuration(0)).toBe("0ms");
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(999)).toBe("999ms");
  });

  it("returns seconds with one decimal for < 60s", () => {
    expect(formatDuration(1000)).toBe("1.0s");
    expect(formatDuration(1500)).toBe("1.5s");
    expect(formatDuration(59999)).toBe("60.0s"); // 59.999s rounds to 60.0s
  });

  it("returns minutes + seconds for >= 60s", () => {
    expect(formatDuration(60000)).toBe("1m 0s");
    expect(formatDuration(90000)).toBe("1m 30s");
    expect(formatDuration(150000)).toBe("2m 30s");
  });

  it("handles large durations", () => {
    expect(formatDuration(3600000)).toBe("60m 0s");
  });
});

// ── parseOutput ─────────────────────────────────────────
describe("parseOutput", () => {
  it("detects XML file content pattern", () => {
    const xml = "<path>/src/main.rs</path>\n<type>file</type>\n<content>fn main() {}</content>";
    const result = parseOutput(xml);
    expect(result.type).toBe("file");
    expect(result.path).toBe("/src/main.rs");
    expect(result.content).toBe("fn main() {}");
  });

  it("detects task_result markdown pattern", () => {
    const md = "<task_result>## Summary\n\nAll done.</task_result>";
    const result = parseOutput(md);
    expect(result.type).toBe("markdown");
    expect(result.content).toBe("## Summary\n\nAll done.");
    expect(result.path).toBe("");
  });

  it("trims whitespace from task_result content", () => {
    const md = "<task_result>\n  Hello  \n</task_result>";
    const result = parseOutput(md);
    expect(result.type).toBe("markdown");
    expect(result.content).toBe("Hello");
  });

  it("returns plain type for regular text", () => {
    const result = parseOutput("Hello world");
    expect(result.type).toBe("plain");
    expect(result.content).toBe("Hello world");
    expect(result.path).toBe("");
  });

  it("handles empty string as plain", () => {
    const result = parseOutput("");
    expect(result.type).toBe("plain");
    expect(result.content).toBe("");
  });

  it("file pattern takes priority over task_result if both present", () => {
    const mixed = "<path>a.txt</path><type>file</type><content>stuff <task_result>md</task_result></content>";
    const result = parseOutput(mixed);
    expect(result.type).toBe("file");
    expect(result.path).toBe("a.txt");
  });
});

// ── guessLanguage ───────────────────────────────────────
describe("guessLanguage", () => {
  it("maps common extensions correctly", () => {
    expect(guessLanguage("/src/app.ts")).toBe("typescript");
    expect(guessLanguage("/src/app.tsx")).toBe("tsx");
    expect(guessLanguage("/src/index.js")).toBe("javascript");
    expect(guessLanguage("/main.py")).toBe("python");
    expect(guessLanguage("/lib.rs")).toBe("rust");
    expect(guessLanguage("/main.go")).toBe("go");
    expect(guessLanguage("/styles.css")).toBe("css");
    expect(guessLanguage("/page.html")).toBe("html");
    expect(guessLanguage("/data.json")).toBe("json");
    expect(guessLanguage("/config.yaml")).toBe("yaml");
    expect(guessLanguage("/config.yml")).toBe("yaml");
    expect(guessLanguage("/config.toml")).toBe("toml");
    expect(guessLanguage("/readme.md")).toBe("markdown");
    expect(guessLanguage("/script.sh")).toBe("bash");
    expect(guessLanguage("/query.sql")).toBe("sql");
  });

  it("detects Dockerfile by filename", () => {
    expect(guessLanguage("/path/Dockerfile")).toBe("dockerfile");
    expect(guessLanguage("Dockerfile")).toBe("dockerfile");
  });

  it("detects Makefile by filename", () => {
    expect(guessLanguage("/path/Makefile")).toBe("makefile");
    expect(guessLanguage("GNUmakefile")).toBe("makefile");
  });

  it("detects .lock files as JSON", () => {
    expect(guessLanguage("package-lock.json.lock")).toBe("json");
    expect(guessLanguage("yarn.lock")).toBe("json");
  });

  it("returns 'text' for unknown extensions", () => {
    expect(guessLanguage("file.xyz")).toBe("text");
    expect(guessLanguage("no-extension")).toBe("text");
  });

  it("is case-insensitive", () => {
    expect(guessLanguage("/SRC/App.TS")).toBe("typescript");
    expect(guessLanguage("DOCKERFILE")).toBe("dockerfile");
  });

  it("handles path with no directory", () => {
    expect(guessLanguage("main.rs")).toBe("rust");
  });
});
