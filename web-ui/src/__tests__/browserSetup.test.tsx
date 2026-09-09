import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { BrowserSetup } from "../browser-panel/BrowserSetup";
import type { BrowserInstallGuide } from "../api/client";

const GUIDE: BrowserInstallGuide = {
  platform: "linux",
  title: "Install Chromium on Linux",
  summary: "Install a Chromium-based browser on the machine running opman.",
  steps: ["Install Chromium.", "Restart opman."],
  commands: [{ label: "Debian / Ubuntu", command: "sudo apt install -y chromium" }],
  env_var: "OPMAN_BROWSER_BIN",
  docs_url: "https://www.chromium.org/",
  issue: { kind: "no_browser" },
};

describe("browser setup state", () => {
  it("shows host-specific setup guidance and recovery", () => {
    render(<BrowserSetup guide={GUIDE} onRetry={vi.fn()} />);

    expect(screen.getByRole("heading", { name: GUIDE.title })).toBeTruthy();
    expect(screen.getByText(GUIDE.summary)).toBeTruthy();
    expect(screen.getByText(GUIDE.commands[0].command)).toBeTruthy();
    expect(screen.getByText(/OPMAN_BROWSER_BIN/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Browser install docs/ })).toHaveAttribute(
      "href",
      GUIDE.docs_url,
    );
  });

  it("explains an unusable browser path and echoes it", () => {
    const path = "/nonexistent/chromium";
    render(
      <BrowserSetup
        guide={{ ...GUIDE, issue: { kind: "override_missing", path } }}
        onRetry={vi.fn()}
      />,
    );

    expect(screen.getByText("The browser path you set needs attention")).toBeTruthy();
    expect(screen.getByText("The configured path does not exist.")).toBeTruthy();
    expect(screen.getByText(path)).toBeTruthy();
  });
});
