import { afterEach, describe, expect, it, vi } from "vitest";

/** Import a fresh copy of base.ts as if the page had been opened at `path`. */
async function loadAt(path: string) {
  window.history.replaceState(null, "", path);
  vi.resetModules();
  return import("../api/base");
}

afterEach(() => {
  window.history.replaceState(null, "", "/");
  vi.resetModules();
});

describe("detectBase", () => {
  it("is empty for home paths", async () => {
    const { detectBase } = await loadAt("/");
    expect(detectBase("/")).toBe("");
    expect(detectBase("/settings")).toBe("");
    expect(detectBase("/remoteish/x")).toBe("");
    expect(detectBase("/remote/")).toBe("");
  });

  it("takes the first segment after /remote/", async () => {
    const { detectBase } = await loadAt("/");
    expect(detectBase("/remote/dev")).toBe("/remote/dev");
    expect(detectBase("/remote/dev/")).toBe("/remote/dev");
    expect(detectBase("/remote/dev/settings")).toBe("/remote/dev");
  });
});

describe("on home", () => {
  it("has no base and the home id", async () => {
    const base = await loadAt("/settings?section=mcp");
    expect(base.BASE).toBe("");
    expect(base.SERVER_ID).toBe("home");
    expect(base.IS_HOME).toBe(true);
  });

  it("builds plain /api URLs", async () => {
    const { apiUrl } = await loadAt("/");
    expect(apiUrl("/state")).toBe("/api/state");
    expect(apiUrl("/file/raw?path=a")).toBe("/api/file/raw?path=a");
  });

  it("keeps storage keys unchanged so existing state survives", async () => {
    const { storageKey } = await loadAt("/");
    expect(storageKey("opman.workspace")).toBe("opman.workspace");
  });

  it("leaves app paths alone", async () => {
    const { appPath, instancePathname } = await loadAt("/kanban");
    expect(appPath("/settings")).toBe("/settings");
    expect(instancePathname()).toBe("/kanban");
  });
});

describe("on a remote instance", () => {
  it("derives BASE and SERVER_ID from the URL", async () => {
    const base = await loadAt("/remote/devbox/settings");
    expect(base.BASE).toBe("/remote/devbox");
    expect(base.SERVER_ID).toBe("devbox");
    expect(base.IS_HOME).toBe(false);
  });

  it("prefixes every API URL with the base", async () => {
    const { apiUrl } = await loadAt("/remote/devbox/");
    expect(apiUrl("/state")).toBe("/remote/devbox/api/state");
    expect(apiUrl("/editor/ws")).toBe("/remote/devbox/api/editor/ws");
  });

  it("namespaces storage keys by server", async () => {
    const { storageKey } = await loadAt("/remote/devbox/");
    expect(storageKey("opman.workspace")).toBe("opman.workspace@devbox");
  });

  it("resolves backend-relative URLs but leaves others", async () => {
    const { resolveBackendUrl } = await loadAt("/remote/devbox/");
    expect(resolveBackendUrl("/api/kanban/asset/t/a.png")).toBe(
      "/remote/devbox/api/kanban/asset/t/a.png",
    );
    expect(resolveBackendUrl("https://example.com/a.png")).toBe("https://example.com/a.png");
  });

  it("keeps app navigation under the base", async () => {
    const { appPath, instancePathname } = await loadAt("/remote/devbox/kanban");
    expect(appPath("/settings?section=acp")).toBe("/remote/devbox/settings?section=acp");
    expect(appPath("/")).toBe("/remote/devbox/");
    expect(appPath("/remote/devbox/kanban")).toBe("/remote/devbox/kanban");
    expect(instancePathname()).toBe("/kanban");
    expect(instancePathname("/remote/devbox")).toBe("/");
  });
});

describe("serverBase", () => {
  it("is empty for home and /remote/<id> otherwise", async () => {
    const { serverBase } = await loadAt("/remote/devbox/");
    expect(serverBase("home")).toBe("");
    expect(serverBase("")).toBe("");
    expect(serverBase("lab-2")).toBe("/remote/lab-2");
  });
});

describe("client on a remote instance", () => {
  it("sends every helper through the remote's proxy", async () => {
    window.history.replaceState(null, "", "/remote/devbox/");
    vi.resetModules();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: () => Promise.resolve("{}"),
      json: () => Promise.resolve({}),
    });
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const client = await import("../api/client");
    await client.apiFetch("/state");
    await client.apiPost("/presence", {});
    await client.homeRequest("GET", "/servers");
    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      "/remote/devbox/api/state",
      "/remote/devbox/api/presence",
      "/api/servers",
    ]);
  });
});
