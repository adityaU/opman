import React from "react";
import { describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CodeReadPreview } from "../tool-call/CodeReadPreview";
import type { InnerCall } from "../tool-call/codeModeParse";

vi.mock("../tool-call/read-viewer", async () => {
  const { chooseViewer } = await import("../tool-call/read-viewer/choice");
  return { chooseViewer, ReadViewer: ({ path }: { path: string }) => <div data-testid="preview">{path}</div> };
});

function inner(tool: string, args: unknown): InnerCall {
  return { tool, input: { text: JSON.stringify(args), truncated: false, bytes: 1 },
    output: { text: "binary metadata", truncated: false, bytes: 1 },
    ok: true, finished: true, durationMs: 1 };
}

describe("files read inside code mode", () => {
  it("previews each supported file type with native and ACP path fields", () => {
    const paths = { path: "image.png", file_path: "video.mp4", filePath: "doc.pdf",
      filename: "sound.mp3", absolute_path: "/tmp/model.stl" };
    for (const [key, path] of Object.entries(paths)) {
      render(<CodeReadPreview call={inner("opman_tools.read_file", { [key]: path })} />);
      expect(screen.getByTestId("preview").textContent).toBe(path);
      cleanup();
    }
  });

  it("ignores unrelated tools", () => {
    const { container } = render(<>
      <CodeReadPreview call={inner("opman_tools.bash", { command: "pwd" })} />
    </>);
    expect(container.textContent).toBe("");
  });

  it("highlights text files and edits inside execute", () => {
    const read = inner("opman_tools.read_file", { path: "a.rs" });
    read.output.text = JSON.stringify({ path: "a.rs", content: "let count = 1;" });
    const { container } = render(<>
      <CodeReadPreview call={read} />
      <CodeReadPreview call={inner("opman_tools.edit_file", {
        path: "a.rs", old_string: "let count = 1;", new_string: "let count = 2;",
      })} />
    </>);
    expect(container.querySelector(".token.keyword")).toHaveTextContent("let");
    expect(container.querySelector(".added .token.number")).toHaveTextContent("2");
  });
});
