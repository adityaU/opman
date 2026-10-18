import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ReadCard } from "../tool-call/readBashCards";
import { EditorOpenProvider } from "../tool-call/EditorOpenContext";
import type { MessagePart } from "../types";

vi.mock("../code-editor/components/PdfCanvasViewer", () => ({
  PdfCanvasViewer: ({ url, alt }: { url: string; alt: string }) => <canvas data-pdf-canvas data-source={url} aria-label={alt} />,
}));

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

function part(path: string, output = "", status = "completed"): MessagePart {
  return { type: "tool", tool: "read", state: { status, input: { file_path: path }, output } } as unknown as MessagePart;
}
const expand = () => fireEvent.click(screen.getByRole("button", { name: /Read/ }));

describe("ReadCard viewers", () => {
  it("mounts nothing while collapsed", () => {
    const { container } = render(<ReadCard part={part("/p/a.png")} />);
    expect(container.querySelector("img, video, object")).toBeNull();
  });

  it("shows an image from the raw route, with open and save actions", () => {
    const open = vi.fn();
    const { container } = render(
      <EditorOpenProvider value={{ open, openWhere: vi.fn() }}><ReadCard part={part("/p/a.png")} /></EditorOpenProvider>,
    );
    expand();
    expect(container.querySelector("img")?.getAttribute("src")).toContain("/file/raw?path=%2Fp%2Fa.png");
    fireEvent.click(screen.getByTitle("Open in pane"));
    expect(open).toHaveBeenCalledWith("/p/a.png");
    expect(screen.getByTitle("Download").getAttribute("href")).toContain("/file/download");
  });

  it("shows a clear state when the image cannot load", () => {
    const { container } = render(<ReadCard part={part("/remote/a.png")} />);
    expand();
    fireEvent.error(container.querySelector("img")!);
    expect(screen.getByText(/File not available/)).toBeTruthy();
  });

  it("uses native controls and metadata preload for video and audio", () => {
    const v = render(<ReadCard part={part("/p/c.mp4")} />);
    expand();
    const video = v.container.querySelector("video")!;
    expect(video.getAttribute("preload")).toBe("metadata");
    expect(video.controls).toBe(true);
    cleanup();
    const a = render(<ReadCard part={part("/p/c.mp3")} />);
    expand();
    expect(a.container.querySelector("audio")?.getAttribute("preload")).toBe("metadata");
  });

  it("checks a PDF exists before loading the shared canvas viewer", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("x", { status: 206, headers: { "content-range": "bytes 0-0/2048" } })));
    const { container } = render(<ReadCard part={part("/p/d.pdf")} />);
    expand();
    await waitFor(() => expect(container.querySelector("canvas[data-pdf-canvas]")?.getAttribute("data-source")).toContain("%2Fp%2Fd.pdf"));
    expect(screen.getByLabelText("d.pdf")).toBeTruthy();
    expect(screen.getByText("2.0 KB")).toBeTruthy();
  });

  it("shows unavailable for a missing PDF", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("no", { status: 404 })));
    render(<ReadCard part={part("/gone/d.pdf")} />);
    expand();
    await waitFor(() => expect(screen.getByText(/File not available/)).toBeTruthy());
  });

  it("renders an inline image without a server fetch", () => {
    const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";
    const { container } = render(<ReadCard part={part("", png)} />);
    expand();
    expect(container.querySelector("img")?.getAttribute("src")).toBe(png);
    expect(screen.queryByTitle("Download")).toBeNull();
  });

  it("uses image bytes reported by ACP even when the file is remote", () => {
    const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";
    const value = part("/remote/a.png");
    value.state!.metadata = { preview: png };
    const { container } = render(<ReadCard part={value} />);
    expand();
    expect(container.querySelector("img")?.getAttribute("src")).toBe(png);
  });

  it("finds the file from ACP locations when arguments have no path", () => {
    const value = part("");
    value.state!.metadata = { locations: ["/p/a.png"] };
    const { container } = render(<ReadCard part={value} />);
    expand();
    expect(container.querySelector("img")?.getAttribute("src")).toContain("%2Fp%2Fa.png");
  });

  it("tries the preview when an agent rejects reading a binary file", () => {
    const value = part("/p/a.png", "", "error");
    value.state!.error = "Binary files cannot be read as text";
    const { container } = render(<ReadCard part={value} />);
    expand();
    expect(screen.getByText("Binary files cannot be read as text")).toBeTruthy();
    expect(container.querySelector("img")?.getAttribute("src")).toContain("%2Fp%2Fa.png");
  });

  it("keeps the text view for code files", () => {
    const { container } = render(<ReadCard part={part("/p/a.rs", "fn main() {}")} />);
    expand();
    expect(container.querySelector(".rv")).toBeNull();
    expect(container.textContent).toContain("fn main");
  });

  it("loads the STL viewer lazily, only when expanded", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("no", { status: 404 })));
    const { container } = render(<ReadCard part={part("/p/m.stl")} />);
    expect(fetch).not.toHaveBeenCalled();
    expand();
    await waitFor(() => expect(screen.getByText(/Could not load this model/)).toBeTruthy());
    expect(container.querySelector(".rv-stl")).toBeTruthy();
  });
});
