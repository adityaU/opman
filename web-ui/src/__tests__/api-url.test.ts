import { describe, expect, it, vi, beforeEach } from "vitest";
import { fetchGitDiff, fetchGitLog, rawFileUrl, searchMessages } from "../api";

function mockFetch(response: Partial<Response> & { json?: () => Promise<unknown> }) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    statusText: "OK",
    json: () => Promise.resolve({}),
    ...response,
  });
}

beforeEach(() => vi.restoreAllMocks());

describe("URL building", () => {
  it("fetchGitDiff builds correct query string", async () => {
    mockFetch({ json: () => Promise.resolve({ diff: "..." }) });
    await fetchGitDiff("src/main.rs", true);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/git/diff?file=src%2Fmain.rs&staged=true"), expect.anything(),
    );
  });

  it("fetchGitDiff with no args has no query string", async () => {
    mockFetch({ json: () => Promise.resolve({ diff: "" }) });
    await fetchGitDiff();
    expect(fetch).toHaveBeenCalledWith("/api/git/diff", expect.anything());
  });

  it("fetchGitLog includes limit", async () => {
    mockFetch({ json: () => Promise.resolve({ commits: [] }) });
    await fetchGitLog(50);
    expect(fetch).toHaveBeenCalledWith("/api/git/log?limit=50", expect.anything());
  });

  it("rawFileUrl includes only the path for cookie auth", () => {
    const url = rawFileUrl("src/foo.rs");
    expect(url).toContain("path=src%2Ffoo.rs");
    expect(url).not.toContain("token=");
  });

  it("searchMessages builds correct query", async () => {
    mockFetch({ json: () => Promise.resolve({ query: "test", results: [], total: 0 }) });
    await searchMessages(0, "hello world", 10);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/project/0/search?q=hello+world&limit=10"), expect.anything(),
    );
  });
});
