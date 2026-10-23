import React from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CodeBlock } from "../message-turn/CodeBlock";
import { codePreviewKind } from "../message-turn/codePreviewKind";
import { HtmlViewer } from "../content-viewers/HtmlViewer";
import { SvgViewer } from "../content-viewers/MarkupViewers";

afterEach(cleanup);

describe("code-block rendered views", () => {
  it("renders Markdown by default and lets the user show the exact source", async () => {
    const code = "# Heading\n\n**Bold text**";
    const { container } = render(<CodeBlock language="markdown" code={code} />);
    expect(await screen.findByRole("heading", { name: "Heading" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Render" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Toggle word wrap" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Code" }));
    expect(container.querySelector(".code-source")).not.toBeNull();
    expect([...container.querySelectorAll(".code-source-text")].map(line => line.textContent).join("")).toBe(code);
    expect(screen.getByRole("button", { name: "Toggle word wrap" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Render" }));
    expect(await screen.findByRole("heading", { name: "Heading" })).toBeVisible();
  });
  it("toggles real wrap state for source code", () => {
    const { container } = render(<CodeBlock language="typescript" code={'const text = "' + "x".repeat(500) + '";'} />);
    expect(container.querySelector(".code-source--wrap")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Toggle word wrap" }));
    expect(container.querySelector(".code-source--nowrap")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Toggle word wrap" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.queryByRole("button", { name: "Render" })).toBeNull();
  });
  it("renders HTML in a script-free isolated frame and keeps authored CSS", async () => {
    const { container } = render(<CodeBlock language="HTML" code={'<!doctype html><html><head><style>h1{color:purple}</style></head><body><h1>Preview</h1><script>alert(1)</script></body></html>'} />);
    const frame = await screen.findByTitle("HTML preview");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(frame.getAttribute("srcdoc")).toContain("h1{color:purple}");
    expect(frame.getAttribute("srcdoc")).not.toContain("<script");
    fireEvent.click(screen.getByRole("button", { name: "Code" }));
    expect(container.querySelector(".token.tag")).not.toBeNull();
  });
  it("renders CSV as a table with escaped cells", async () => {
    render(<CodeBlock language="csv" code={'Name,Count\n"<script>",3'} />);
    expect(await screen.findByRole("columnheader", { name: "Name" })).toBeVisible();
    expect(screen.getByRole("cell", { name: "<script>" })).toBeVisible();
  });
  it("isolates SVG styling in an image and removes event handlers", () => {
    const { container } = render(<SvgViewer content={'<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><style>body{display:none}</style><circle r="10"/></svg>'} />);
    const image = screen.getByRole("img", { name: "SVG preview" });
    expect(image.getAttribute("src")).toMatch(/^data:image\/svg\+xml/);
    expect(decodeURIComponent(image.getAttribute("src") ?? "")).not.toContain("onload");
    expect(container.querySelector("svg")).toBeNull();
    expect(container.querySelector("style")).toBeNull();
  });
  it("renders another HTML document after content updates", async () => {
    const view = render(<HtmlViewer content="<p>First</p>" />);
    view.rerender(<HtmlViewer content="<p>Second</p>" />);
    await waitFor(() => expect(screen.getByTitle("HTML preview").getAttribute("srcdoc")).toContain("Second"));
  });
  it.each(["markdown", "md", "html", "htm", "svg", "csv", "mermaid", "mmd"])("offers a rendered view for %s", language => {
    expect(codePreviewKind(language)).not.toBeNull();
  });
});
