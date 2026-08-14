/**
 * The browser card reads the tools' own plain-text output.
 *
 * That format is deliberate — wrapping results in JSON for the card's benefit
 * would put tokens back on every call the model makes — so these tests pin the
 * parsing, including that a shape it does not recognise still renders rather
 * than throwing the card away.
 */
import { describe, it, expect } from "vitest";
import {
  browserAction,
  hostOf,
  isBrowserTool,
  parsePageResult,
  parsePanes,
  parseScreenshot,
  pathOf,
} from "../tool-call/browserParse";
import { parseOutline, actionableCount } from "../tool-call/browserOutline";
import { planBrowserOpen } from "../workspace/browserOpen";
import { browserIdForProject } from "../api/browser";
import { asPaneId, type PaneNode } from "../workspace/types";

const OUTLINE = `main
 h1 "Welcome"
  text "The quick brown fox."
  textbox "Search" [ref=e2] value="hi"
  checkbox "Remember me" [ref=e3] checked
  button "Go" [ref=e4]`;

const PAGE = `Example\nhttps://example.com/docs\n\n${OUTLINE}`;

describe("browser tool detection", () => {
  it("claims every browser tool, prefixed or not", () => {
    for (const name of [
      "browser_snapshot",
      "mcp__browser__browser_click",
      "browser_read_text",
    ]) {
      expect(isBrowserTool(name)).toBe(true);
    }
  });

  it("does not claim unrelated tools", () => {
    for (const name of ["read", "web_fetch", "kanban_get_task"]) {
      expect(isBrowserTool(name)).toBe(false);
    }
  });

  it("names the action, however the runner prefixed it", () => {
    expect(browserAction("mcp__browser__browser_read_text")).toBe("read");
    expect(browserAction("browser_snapshot")).toBe("snapshot");
    expect(browserAction("browser_press_key")).toBe("key");
    expect(browserAction("browser_list_panes")).toBe("panes");
    // `read_text` must not be mistaken for a snapshot, nor `list_panes` for a click.
    expect(browserAction("browser_click")).toBe("click");
  });
});

describe("page results", () => {
  it("splits title, url and body", () => {
    const page = parsePageResult(PAGE);
    expect(page?.title).toBe("Example");
    expect(page?.url).toBe("https://example.com/docs");
    expect(page?.body).toBe(OUTLINE);
    expect(page?.truncated).toBe(false);
  });

  it("records truncation and strips its footer from the body", () => {
    const page = parsePageResult(`${PAGE}\n\n[outline truncated — raise max_nodes]`);
    expect(page?.truncated).toBe(true);
    expect(page?.body).toBe(OUTLINE);
  });

  it("keeps text it cannot parse rather than dropping the result", () => {
    const page = parsePageResult("something unexpected entirely");
    expect(page?.body).toBe("something unexpected entirely");
    expect(page?.url).toBe("");
  });

  it("has nothing to show before the call answers", () => {
    expect(parsePageResult(null)).toBeNull();
  });
});

describe("outline rows", () => {
  it("separates depth, role, name and ref", () => {
    const rows = parseOutline(OUTLINE);
    const search = rows.find((row) => row.ref === "e2");
    expect(search?.role).toBe("textbox");
    expect(search?.name).toBe("Search");
    expect(search?.note).toContain('value="hi"');
    expect(rows[0]).toMatchObject({ depth: 0, role: "main" });
    expect(rows[1].depth).toBe(1);
  });

  it("counts only what the agent could act on", () => {
    // Three refs among six rows: the landmark, heading and text are not targets.
    expect(actionableCount(parseOutline(OUTLINE))).toBe(3);
  });

  it("survives a line in no known shape", () => {
    expect(() => parseOutline("!!! (unparseable")).not.toThrow();
    expect(parseOutline("!!! (unparseable")).toHaveLength(1);
  });
});

describe("screenshots and sessions", () => {
  it("splits the header from the base64 payload", () => {
    const shot = parseScreenshot("Screenshot of https://a.example (base64 JPEG):\n/9j/4AAQ");
    expect(shot?.data).toBe("/9j/4AAQ");
    expect(shot?.url).toContain("a.example");
  });

  it("refuses output that is not a screenshot", () => {
    expect(parseScreenshot(PAGE)).toBeNull();
  });

  it("reads one session per line", () => {
    const rows = parsePanes("/repo/one  Docs  https://d.example\n/repo/two  App  http://localhost:3000");
    expect(rows).toHaveLength(2);
    expect(rows[1]).toEqual({
      project: "/repo/two",
      title: "App",
      url: "http://localhost:3000",
    });
  });

  it("shows nothing for the empty-state message", () => {
    expect(parsePanes("No browsers are open. Call browser_open with a URL")).toHaveLength(0);
  });
});

describe("url display", () => {
  it("leads with the host, which is what identifies a page", () => {
    expect(hostOf("https://github.com/anthropics/x?a=1")).toBe("github.com");
    expect(pathOf("https://github.com/anthropics/x?a=1")).toBe("/anthropics/x?a=1");
  });

  it("shows no path for a bare host", () => {
    expect(pathOf("https://example.com/")).toBe("");
  });

  it("degrades rather than throwing on a malformed url", () => {
    expect(hostOf("not a url")).toBe("not a url");
  });
});

describe("revealing an agent's browser", () => {
  const leaf = (id: string, widget: PaneNode["widget"]): PaneNode => ({
    type: "leaf",
    id: asPaneId(id),
    widget,
  });
  const browser = (project: string) =>
    ({ kind: "browser", projectPath: project, browserId: browserIdForProject(project), url: null }) as const;

  it("reuses the pane already showing that project's browser", () => {
    const panes = [
      leaf("p1", { kind: "chat", projectPath: "/repo", sessionId: null, engine: null }),
      leaf("p2", browser("/repo")),
    ];
    const plan = planBrowserOpen("/repo", "https://x.example", panes, asPaneId("p1"));
    expect(plan.action).toBe("place");
    expect(plan.pane).toBe("p2");
  });

  it("fills an empty focused pane rather than splitting it", () => {
    const panes = [leaf("p1", null)];
    const plan = planBrowserOpen("/repo", "https://x.example", panes, asPaneId("p1"));
    expect(plan).toMatchObject({ action: "place", pane: "p1" });
  });

  it("splits a column beside the work when every pane is busy", () => {
    const panes = [leaf("p1", { kind: "git", projectPath: "/repo" })];
    const plan = planBrowserOpen("/repo", "https://x.example", panes, asPaneId("p1"));
    expect(plan).toMatchObject({ action: "split", pane: "p1" });
  });

  it("does not reuse another project's browser", () => {
    const panes = [leaf("p1", browser("/other"))];
    const plan = planBrowserOpen("/repo", "https://x.example", panes, asPaneId("p1"));
    expect(plan.action).toBe("split");
  });

  it("carries the url the agent went to", () => {
    const panes = [leaf("p1", null)];
    const plan = planBrowserOpen("/repo", "https://x.example/page", panes, asPaneId("p1"));
    expect(plan.widget).toMatchObject({
      kind: "browser",
      projectPath: "/repo",
      url: "https://x.example/page",
    });
  });
});
