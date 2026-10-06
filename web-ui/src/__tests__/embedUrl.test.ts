/** The embed URL contract, and the width-query rewrite that keeps embeds on desktop. */
import { describe, it, expect } from "vitest";
import { embedUrl, parseEmbedSearch, widgetKey } from "../embed/embedUrl";
import { wideMediaText } from "../embed/wideViewport";
import type { WidgetState } from "../workspace/types";

const chat: WidgetState = { kind: "chat", projectPath: "/r p", sessionId: "s1", engine: null, server: "dev" };

describe("embedUrl", () => {
  it("is the server's base with the widget, minus server, url-encoded", () => {
    const url = embedUrl("dev", chat);
    expect(url.startsWith("/remote/dev/?embed=")).toBe(true);
    const raw = decodeURIComponent(url.split("?embed=")[1]);
    expect(JSON.parse(raw)).toEqual({ kind: "chat", projectPath: "/r p", sessionId: "s1", engine: null });
  });

  it("uses no prefix for home", () => {
    expect(embedUrl("home", chat).startsWith("/?embed=")).toBe(true);
  });

  it("round-trips through the page's search string", () => {
    const search = `?${embedUrl("dev", chat).split("?")[1]}`;
    expect(parseEmbedSearch(search)).toEqual({ kind: "chat", projectPath: "/r p", sessionId: "s1", engine: null });
  });

  it("is null for a normal load, bad JSON, or something that is not a widget", () => {
    expect(parseEmbedSearch("")).toBeNull();
    expect(parseEmbedSearch("?embed=%7Bnope")).toBeNull();
    expect(parseEmbedSearch(`?embed=${encodeURIComponent(JSON.stringify({ kind: "nope" }))}`)).toBeNull();
  });

  it("drops a server smuggled into the parameter: inside, it is the own server", () => {
    const search = `?embed=${encodeURIComponent(JSON.stringify({ kind: "git", projectPath: "/r", server: "x" }))}`;
    expect(parseEmbedSearch(search)).toEqual({ kind: "git", projectPath: "/r" });
  });

  it("keys widgets without their server", () => {
    expect(widgetKey(chat)).toBe(widgetKey({ ...chat, server: undefined }));
  });
});

describe("wideMediaText", () => {
  it("never matches max-width and always matches min-width", () => {
    expect(wideMediaText("(max-width: 768px)")).toBe("not all");
    expect(wideMediaText("(max-width: 768px) and (display-mode: standalone)")).toBe("not all");
    expect(wideMediaText("(min-width: 769px)")).toBe("all");
    expect(wideMediaText("screen and (min-width: 769px)")).toBe("screen");
  });

  it("leaves every other query alone", () => {
    expect(wideMediaText("(prefers-reduced-motion: reduce)")).toBeNull();
    expect(wideMediaText("print")).toBeNull();
  });
});
