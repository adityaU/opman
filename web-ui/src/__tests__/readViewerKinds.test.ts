import { describe, expect, it } from "vitest";
import { FileKind, fileKindOf, extensionOf, formatBytes, baseName, isRenderable } from "../tool-call/read-viewer/fileKind";
import { inlineImageUrl, sourceFor, downloadHref } from "../tool-call/read-viewer/source";
import { chooseViewer } from "../tool-call/read-viewer/choice";
import { isReadTool } from "../tool-call/readBashCards";

describe("fileKindOf", () => {
  it.each([
    ["a.png", FileKind.Image], ["A.JPG", FileKind.Image], ["x.webp", FileKind.Image], ["x.avif", FileKind.Image],
    ["a.svg", FileKind.Svg], ["v.MP4", FileKind.Video], ["v.mov", FileKind.Video], ["v.ogv", FileKind.Video],
    ["s.mp3", FileKind.Audio], ["s.flac", FileKind.Audio], ["d.pdf", FileKind.Pdf], ["/p/m.STL", FileKind.Stl],
    ["main.rs", FileKind.Other], ["Makefile", FileKind.Other], [".png", FileKind.Other], ["", FileKind.Other],
  ])("%s", (path, kind) => expect(fileKindOf(path)).toBe(kind));

  it("uses only the last segment for the extension", () => {
    expect(extensionOf("/a.b/c")).toBe("");
    expect(extensionOf("C:\\x\\y.PNG")).toBe("png");
  });
  it("knows what is renderable", () => {
    expect(isRenderable(FileKind.Pdf)).toBe(true);
    expect(isRenderable(FileKind.Other)).toBe(false);
  });
  it("formats sizes and names", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(-1)).toBe("");
    expect(baseName("/a/b/c.txt")).toBe("c.txt");
  });
});

describe("sources", () => {
  const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";
  it("spots an inline image only when the whole output is one", () => {
    expect(inlineImageUrl(png)).toBe(png);
    expect(inlineImageUrl(`${png}\nmore text`)).toBeNull();
    expect(inlineImageUrl("plain output")).toBeNull();
  });
  it("builds a server url for a path, encoded", () => {
    const s = sourceFor("/tmp/a b.png", "");
    expect(s?.kind).toBe("server");
    expect(s?.url).toContain("/api/file/raw?path=%2Ftmp%2Fa%20b.png");
    expect(s && downloadHref(s)).toContain("/api/file/download?path=");
  });
  it("has no source without a path or inline data", () => {
    expect(sourceFor("", "text")).toBeNull();
  });
  it("inline data has no download link", () => {
    const s = sourceFor("", png);
    expect(s?.kind).toBe("inline");
    expect(s && downloadHref(s)).toBeNull();
  });
});

describe("chooseViewer", () => {
  it("picks a viewer from the extension", () => {
    expect(chooseViewer("/x/m.stl", "binary…")?.kind).toBe(FileKind.Stl);
    expect(chooseViewer("clip.webm", "")?.kind).toBe(FileKind.Video);
  });
  it("falls back for text and unknown files", () => {
    expect(chooseViewer("src/main.rs", "fn main(){}")).toBeNull();
    expect(chooseViewer("", "hello")).toBeNull();
  });
  it("prefers an inline image, even with no path", () => {
    const c = chooseViewer("", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==");
    expect(c?.kind).toBe(FileKind.Image);
    expect(c?.source.kind).toBe("inline");
  });
  it.each([
    ["audio/wav", FileKind.Audio], ["video/mp4", FileKind.Video],
    ["application/pdf", FileKind.Pdf], ["model/stl", FileKind.Stl],
  ])("uses the inline %s media type", (mime, kind) => {
    expect(chooseViewer("", `data:${mime};base64,YWJjZA==`)?.kind).toBe(kind);
  });
});

describe("isReadTool across runners", () => {
  it.each(["Read", "read", "read_file", "mcp__opman-tools__read_file", "mcp__fs__read_text_file", "view", "cat", "mcp__x__open_file", "ReadMcpResource", "view_image"])(
    "%s is a read", (n) => expect(isReadTool(n)).toBe(true));
  it.each(["bash", "edit", "TodoRead", "neovim_read", "notes_read", "write_file", "overview"])(
    "%s is not", (n) => expect(isReadTool(n)).toBe(false));
});
