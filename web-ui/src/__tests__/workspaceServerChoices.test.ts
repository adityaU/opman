/** The opener's project list, grouped by server once there is more than one. */
import { describe, it, expect } from "vitest";
import type { ServerInfo } from "../api/servers";
import {
  advance,
  decodeProjectChoice,
  EMPTY_DRAFT,
  projectChoiceValue,
  retreat,
  toWidget,
} from "../workspace/opener/steps";
import { choicesForStep } from "../workspace/opener/choices";
import { groupedProjectChoices, remoteSessionChoices } from "../workspace/servers/serverChoices";
import type { Load, RemoteProject } from "../workspace/servers/remoteCatalog";

const server = (id: string, status: ServerInfo["status"] = "ok"): ServerInfo => ({
  id, name: id === "home" ? "Home" : `Box ${id}`, url: id === "home" ? null : `https://${id}`, base: "", status,
});
const own = { id: "home", projects: [{ path: "/a", name: "a" }, { path: "/b", name: "b" }] };
const ok = (projects: RemoteProject[]): Load<readonly RemoteProject[]> => ({ status: "ok", data: projects });

describe("groupedProjectChoices", () => {
  it("is the old flat list with one server", () => {
    expect(groupedProjectChoices(own, [server("home")], new Map())).toEqual([
      { value: "/a", label: "a", hint: "/a" },
      { value: "/b", label: "b", hint: "/b" },
    ]);
    expect(groupedProjectChoices(own, [], new Map())).toHaveLength(2);
  });

  it("groups every server's projects, own first", () => {
    const remote = new Map([["dev", ok([{ path: "/x", name: "x", index: 0 }])]]);
    const rows = groupedProjectChoices(own, [server("home"), server("dev")], remote);
    expect(rows.map((row) => [row.group, row.label])).toEqual([
      ["Home", "a"], ["Home", "b"], ["Box dev", "x"],
    ]);
    expect(rows[0].groupNote).toBe("this server");
    expect(decodeProjectChoice(rows[2].value!)).toEqual({ server: "dev", path: "/x" });
    expect(rows[2].disabled).toBe(false);
  });

  it("lists unreachable, failing and loading servers as one disabled row", () => {
    const remote = new Map<string, Load<readonly RemoteProject[]>>([
      ["fail", { status: "error", error: "sign-in failed", data: null }],
      ["slow", { status: "loading", data: null }],
    ]);
    const rows = groupedProjectChoices(
      own,
      [server("home"), server("down", "unreachable"), server("fail"), server("slow")],
      remote,
    );
    const status = rows.filter((row) => row.disabled);
    expect(status.map((row) => [row.group, row.label, row.groupNote])).toEqual([
      ["Box down", "Not available", "unreachable"],
      ["Box fail", "Not available", "sign-in failed"],
      ["Box slow", "Loading projects…", ""],
    ]);
    expect(new Set(status.map((row) => row.value)).size).toBe(3);
  });

  it("keeps the last known projects, disabled, when a refresh fails", () => {
    const remote = new Map([["dev", {
      status: "error" as const, error: "unreachable", data: [{ path: "/x", name: "x", index: 0 }],
    }]]);
    const rows = groupedProjectChoices(own, [server("home"), server("dev")], remote);
    expect(rows[2]).toMatchObject({ label: "x", disabled: true, groupNote: "unreachable" });
  });
});

describe("opener draft with a server", () => {
  it("encodes only other servers, so own values are bare paths", () => {
    expect(projectChoiceValue(null, "/a")).toBe("/a");
    expect(decodeProjectChoice("/a")).toEqual({ server: null, path: "/a" });
    expect(decodeProjectChoice(projectChoiceValue("dev", "/a/b"))).toEqual({ server: "dev", path: "/a/b" });
  });

  it("carries the server from the project step onto the widget", () => {
    let draft = advance(EMPTY_DRAFT, "git");
    draft = advance(draft, projectChoiceValue("dev", "/x"));
    expect(toWidget(draft)).toEqual({ kind: "git", projectPath: "/x", server: "dev" });
    expect(retreat(draft)).toMatchObject({ projectPath: null, server: null });
    expect(toWidget(advance(advance(EMPTY_DRAFT, "git"), "/a"))).toEqual({ kind: "git", projectPath: "/a" });
  });

  it("lists a remote project's sessions and defers its shells to the pane", () => {
    const draft = { ...EMPTY_DRAFT, kind: "chat" as const, projectPath: "/x", server: "dev" };
    const sources = {
      projectChoices: [],
      sessionsFor: () => [{ id: "local", title: "local", updated: 1 }],
      remoteSessions: () => remoteSessionChoices({ status: "ok", data: [{ id: "r1", title: "R", updated: 2 }] }),
      shells: [],
    };
    expect(choicesForStep("session", draft, sources).map((c) => c.value)).toEqual([null, "r1"]);
    const shells = choicesForStep("shell", { ...draft, kind: "terminal" }, sources);
    expect(shells).toHaveLength(1);
    expect(shells[0].value).toBeNull();
  });

  it("shows a disabled loading row until a remote project's sessions arrive", () => {
    const rows = remoteSessionChoices(undefined);
    expect(rows[0].value).toBeNull();
    expect(rows[1]).toMatchObject({ label: "Loading sessions…", disabled: true });
  });
});
