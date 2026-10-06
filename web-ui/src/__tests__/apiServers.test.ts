import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { addServer, listServers, removeServer, updateServer } from "../api/servers";
import { refreshServers, resetServersCache, useServers } from "../hooks/useServers";

const HOME = { id: "home", name: "Home", url: null, base: "", status: "ok" };
const DEV = {
  id: "devbox",
  name: "Dev box",
  url: "https://dev.example.com",
  base: "/remote/devbox",
  status: "unreachable",
};

function respond(status: number, body?: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: "",
    text: () => Promise.resolve(body === undefined ? "" : JSON.stringify(body)),
    json: () => Promise.resolve(body),
  };
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  fetchMock = vi.fn().mockResolvedValue(respond(200, [HOME, DEV]));
  globalThis.fetch = fetchMock as unknown as typeof fetch;
  resetServersCache();
});

afterEach(() => vi.restoreAllMocks());

describe("servers API", () => {
  it("lists from home's absolute /api/servers", async () => {
    const servers = await listServers();
    expect(servers.map((s) => s.id)).toEqual(["home", "devbox"]);
    expect(fetchMock).toHaveBeenCalledWith("/api/servers", expect.objectContaining({ method: "GET" }));
  });

  it("posts a new server", async () => {
    fetchMock.mockResolvedValueOnce(respond(200, DEV));
    const body = { name: "Dev box", url: "https://dev.example.com", username: "a", password: "b" };
    await addServer(body);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/servers");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual(body);
  });

  it("surfaces the backend's validation message", async () => {
    fetchMock.mockResolvedValueOnce(respond(400, { error: "login failed: bad password" }));
    await expect(addServer({ name: "x", url: "http://x" })).rejects.toThrow(
      "login failed: bad password",
    );
  });

  it("patches and deletes by id", async () => {
    fetchMock.mockResolvedValueOnce(respond(200, DEV));
    await updateServer("devbox", { name: "Dev" });
    expect(fetchMock.mock.calls[0][0]).toBe("/api/servers/devbox");
    expect(fetchMock.mock.calls[0][1].method).toBe("PATCH");

    fetchMock.mockResolvedValueOnce(respond(204));
    await removeServer("devbox");
    expect(fetchMock.mock.calls[1][0]).toBe("/api/servers/devbox");
    expect(fetchMock.mock.calls[1][1].method).toBe("DELETE");
  });
});

describe("useServers", () => {
  it("fetches once and shares the list between callers", async () => {
    const first = renderHook(() => useServers());
    const second = renderHook(() => useServers());
    await waitFor(() => expect(first.result.current.servers).toHaveLength(2));
    expect(second.result.current.servers).toHaveLength(2);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(first.result.current.multi).toBe(true);
    expect(first.result.current.current?.id).toBe("home");
  });

  it("is single-server until the list says otherwise", async () => {
    fetchMock.mockResolvedValue(respond(200, [HOME]));
    const { result } = renderHook(() => useServers());
    expect(result.current.multi).toBe(false);
    await waitFor(() => expect(result.current.servers).toHaveLength(1));
    expect(result.current.multi).toBe(false);
  });

  it("refresh updates every caller", async () => {
    const { result } = renderHook(() => useServers());
    await waitFor(() => expect(result.current.servers).toHaveLength(2));
    fetchMock.mockResolvedValueOnce(respond(200, [HOME]));
    await act(() => refreshServers());
    expect(result.current.servers).toHaveLength(1);
  });

  it("keeps the last list and reports an error when the fetch fails", async () => {
    const { result } = renderHook(() => useServers());
    await waitFor(() => expect(result.current.servers).toHaveLength(2));
    fetchMock.mockResolvedValueOnce(respond(500, { error: "boom" }));
    await act(() => result.current.refresh());
    expect(result.current.error).toBe("boom");
    expect(result.current.servers).toHaveLength(2);
  });
});
