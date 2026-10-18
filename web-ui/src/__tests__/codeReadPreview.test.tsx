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

  it("ignores other tools and text files", () => {
    const { container } = render(<>
      <CodeReadPreview call={inner("opman_tools.write_file", { path: "a.png" })} />
      <CodeReadPreview call={inner("opman_tools.read_file", { path: "a.rs" })} />
    </>);
    expect(container.textContent).toBe("");
  });
});
