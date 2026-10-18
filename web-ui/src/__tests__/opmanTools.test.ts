import { describe, expect, it } from "vitest";
import {
  opmanDisplayName,
  opmanToolLeaf,
  readableOutput,
  withReadableOutput,
} from "../tool-call/opmanTools";
import type { MessagePart } from "../types";

const json = (value: unknown) => JSON.stringify(value);

describe("names", () => {
  it("shows the eight tools under the built-ins' names", () => {
    expect(opmanDisplayName("mcp__opman-tools__grep")).toBe("Grep");
    expect(opmanDisplayName("mcp__opman-tools__read_file")).toBe("Read");
    expect(opmanDisplayName("mcp__opman-tools__list_dir")).toBe("LS");
    expect(opmanDisplayName("opman-tools_web_fetch")).toBe("WebFetch");
  });

  it("shows the others as plain words", () => {
    expect(opmanDisplayName("mcp__opman-tools__scheduler_list")).toBe("Scheduler list");
    expect(opmanDisplayName("mcp__opman-tools__wait")).toBe("Wait");
  });

  it("leaves every other tool alone", () => {
    expect(opmanDisplayName("Bash")).toBeNull();
    expect(opmanDisplayName("mcp__kanban__kanban_get_task")).toBeNull();
    expect(opmanToolLeaf("mcp__opman-tools__wait")).toBeNull();
    expect(opmanToolLeaf("mcp__opman-tools__bash")).toBe("bash");
  });
});

describe("readableOutput", () => {
  it("bash: output, then stderr, then a non-zero exit", () => {
    const out = readableOutput("bash", json({ stdout: "hi\n", stderr: "warn\n", exit_code: 2, timed_out: false }));
    expect(out).toBe("hi\n[stderr]\nwarn\n[exit code 2]");
  });

  it("bash: a clean run is only its output", () => {
    expect(readableOutput("bash", json({ stdout: "ok\n", stderr: "", exit_code: 0, timed_out: false }))).toBe("ok");
  });

  it("bash: a background job says where its output is", () => {
    const out = readableOutput("bash", json({
      background: true,
      job: { id: "7-1", pid: 9, stdout_file: "/t/a.out", stderr_file: "/t/a.err", exit_file: "/t/a.exit" },
    }));
    expect(out).toContain("moved to the background");
    expect(out).toContain("/t/a.out");
    expect(out).toContain("/t/a.exit");
  });

  it("read_file: the file in the shape the read card highlights", () => {
    const out = readableOutput("read_file", json({ path: "a.rs", content: "fn main() {}", has_more: false }));
    expect(out).toBe("<path>a.rs</path>\n<content>fn main() {}</content>");
  });

  it("grep: one line per hit in each mode", () => {
    const hit = { path: "a.rs", line: 3, text: "let x", count: 2 };
    expect(readableOutput("grep", json({ mode: "content", results: [hit], truncated: false }))).toBe("a.rs:3:let x");
    expect(readableOutput("grep", json({ mode: "files", results: [hit], truncated: true }))).toBe("a.rs\n[more results not shown]");
    expect(readableOutput("grep", json({ mode: "count", results: [hit], truncated: false }))).toBe("a.rs: 2");
  });

  it("list_dir and glob: names, folders marked", () => {
    expect(readableOutput("list_dir", json({ entries: [{ kind: "dir", name: "src" }, { kind: "file", name: "a.md" }] }))).toBe("src/\na.md");
    expect(readableOutput("glob", json({ files: ["a.rs", "b.rs"] }))).toBe("a.rs\nb.rs");
  });

  it("write_file, edit_file and web_fetch", () => {
    expect(readableOutput("write_file", json({ path: "a", bytes: 3, created: true }))).toBe("Created a (3 bytes)");
    expect(readableOutput("edit_file", json({ path: "a", replacements: 1, diff: "-x\n+y" }))).toBe("Edited a: 1 change\n-x\n+y");
    expect(readableOutput("web_fetch", json({ content: "page" }))).toBe("page");
  });

  it("text that is not that JSON comes back unchanged", () => {
    expect(readableOutput("bash", "cannot start the shell")).toBe("cannot start the shell");
    expect(readableOutput("grep", "[1,2]")).toBe("[1,2]");
    expect(readableOutput("unknown", json({ a: 1 }))).toBe(json({ a: 1 }));
  });
});

describe("withReadableOutput", () => {
  const part = (tool: string, output: string) =>
    ({ type: "tool", tool, state: { status: "completed", output } }) as unknown as MessagePart;

  it("rewrites an opman-tools result and keeps the rest of the part", () => {
    const next = withReadableOutput(part("mcp__opman-tools__glob", json({ files: ["a"] })));
    expect(next.state?.output).toBe("a");
    expect(next.state?.status).toBe("completed");
  });

  it("returns other tools' parts as they are", () => {
    const other = part("Bash", json({ files: ["a"] }));
    expect(withReadableOutput(other)).toBe(other);
  });
});
