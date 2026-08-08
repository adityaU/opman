import { describe, it, expect } from "vitest";
import { nextFileOpenSeq, planFileOpen, projectForFile } from "../workspace/fileOpen";
import { asPaneId } from "../workspace/types";
import type { FileOpenRequest, PaneNode, WidgetState } from "../workspace/types";
import type { WorkspaceProject } from "../workspace/DesktopWorkspace";

const REPO = "/home/dev/repo";
const OTHER = "/home/dev/other";
const PROJECTS: WorkspaceProject[] = [
  { path: REPO, name: "repo" },
  { path: OTHER, name: "other" },
];

const pane = (id: string, widget: WidgetState | null): PaneNode => ({
  type: "leaf",
  id: asPaneId(id),
  widget,
});

const chat = (projectPath: string): WidgetState => ({
  kind: "chat",
  projectPath,
  sessionId: null,
  engine: null,
});
const files = (projectPath: string, open: FileOpenRequest | null = null): WidgetState => ({
  kind: "files",
  projectPath,
  open,
});

const request = (path: string, line: number | null = null): FileOpenRequest => ({
  path,
  line,
  seq: 1,
});

describe("projectForFile", () => {
  it("picks the longest project root containing the file", () => {
    const nested = [...PROJECTS, { path: `${REPO}/packages/ui`, name: "ui" }];
    expect(projectForFile(`${REPO}/packages/ui/src/a.ts`, nested, undefined)).toBe(`${REPO}/packages/ui`);
    expect(projectForFile(`${REPO}/src/a.ts`, nested, undefined)).toBe(REPO);
  });

  it("does not match a root that is only a string prefix", () => {
    expect(projectForFile(`${REPO}-backup/a.ts`, PROJECTS, undefined)).toBe(REPO);
    expect(projectForFile(`${REPO}-backup/a.ts`, PROJECTS, OTHER)).toBe(OTHER);
  });

  it("falls back to the asking pane's project for a relative path", () => {
    expect(projectForFile("src/a.ts", PROJECTS, OTHER)).toBe(OTHER);
  });

  it("falls back to the first project when nothing else answers", () => {
    expect(projectForFile("src/a.ts", PROJECTS, undefined)).toBe(REPO);
  });
});

describe("planFileOpen", () => {
  it("reuses the files pane already showing that project", () => {
    const panes = [pane("a", chat(REPO)), pane("b", files(REPO))];
    const open = request(`${REPO}/src/a.ts`, 12);

    expect(planFileOpen(open, panes, asPaneId("a"), PROJECTS)).toEqual({
      action: "place",
      pane: asPaneId("b"),
      widget: files(REPO, open),
    });
  });

  it("reuses the files pane even when it is the focused one", () => {
    const panes = [pane("a", files(REPO, request("old.ts")))];
    const open = request(`${REPO}/src/a.ts`);

    expect(planFileOpen(open, panes, asPaneId("a"), PROJECTS)).toEqual({
      action: "place",
      pane: asPaneId("a"),
      widget: files(REPO, open),
    });
  });

  it("splits beside the focused pane when no files pane is open", () => {
    const panes = [pane("a", chat(REPO))];
    const open = request(`${REPO}/src/a.ts`);

    expect(planFileOpen(open, panes, asPaneId("a"), PROJECTS)).toEqual({
      action: "split",
      pane: asPaneId("a"),
      widget: files(REPO, open),
    });
  });

  it("fills the focused pane instead of splitting when it is empty", () => {
    const panes = [pane("a", chat(REPO)), pane("b", null)];
    const open = request(`${REPO}/src/a.ts`);

    expect(planFileOpen(open, panes, asPaneId("b"), PROJECTS)).toEqual({
      action: "place",
      pane: asPaneId("b"),
      widget: files(REPO, open),
    });
  });

  it("does not hand a file to a files pane rooted in another project", () => {
    const panes = [pane("a", chat(OTHER)), pane("b", files(REPO))];
    const open = request(`${OTHER}/src/a.ts`);

    expect(planFileOpen(open, panes, asPaneId("a"), PROJECTS)).toEqual({
      action: "split",
      pane: asPaneId("a"),
      widget: files(OTHER, open),
    });
  });

  it("resolves a relative path against the pane that asked", () => {
    const panes = [pane("a", chat(OTHER)), pane("b", files(REPO))];
    const open = request("src/a.ts");

    expect(planFileOpen(open, panes, asPaneId("a"), PROJECTS)).toEqual({
      action: "split",
      pane: asPaneId("a"),
      widget: files(OTHER, open),
    });
  });
});

describe("nextFileOpenSeq", () => {
  it("rises on every call, so asking for the same path twice is two requests", () => {
    const first = nextFileOpenSeq();
    const second = nextFileOpenSeq();
    expect(second).toBeGreaterThan(first);
  });
});
