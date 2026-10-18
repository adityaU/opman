import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserPanel } from "../browser-panel/BrowserTabs";

vi.mock("../browser-panel/BrowserPanel", () => ({
  BrowserTabPanel: ({ paneId, url }: { paneId: string; url: string | null }) =>
    <div data-testid="native-tab" data-id={paneId}>{url ?? "about:blank"}</div>,
}));
vi.mock("../api/browser", () => ({ browserClose: vi.fn(async () => {}) }));

afterEach(() => { cleanup(); localStorage.clear(); });

const props = { paneId: "proj:/repo", project: "/repo", url: "https://example.com/",
  reveal: 0, focused: true, onUrlChanged: vi.fn() };

describe("browser internal tabs", () => {
  it("keeps separate native tab IDs and switches back to an existing tab", () => {
    render(<BrowserPanel {...props} />);
    expect(screen.getByTestId("native-tab").dataset.id).toBe(props.paneId);
    fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    expect(screen.getByTestId("native-tab").dataset.id).not.toBe(props.paneId);
    fireEvent.click(screen.getByRole("tab", { name: "example.com" }));
    expect(screen.getByTestId("native-tab").dataset.id).toBe(props.paneId);
  });

  it("opens a requested link in a new tab and reuses it on repeated requests", () => {
    const view = render(<BrowserPanel {...props} />);
    view.rerender(<BrowserPanel {...props} url="https://other.example/" reveal={1} />);
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    expect(screen.getByTestId("native-tab").textContent).toBe("https://other.example/");
    view.rerender(<BrowserPanel {...props} url="https://other.example/" reveal={2} />);
    expect(screen.getAllByRole("tab")).toHaveLength(2);
  });

  it("restores the selected tab after the pane remounts", () => {
    const view = render(<BrowserPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
    const active = screen.getByTestId("native-tab").dataset.id;
    view.unmount();
    render(<BrowserPanel {...props} url={null} />);
    expect(screen.getByTestId("native-tab").dataset.id).toBe(active);
  });

  it("keeps agent navigation on the canonical shared tab", () => {
    const view = render(<BrowserPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
    fireEvent(window, new CustomEvent("opman:browser-agent-page", {
      detail: { projectPath: "/repo", url: "https://agent.example/" },
    }));
    view.rerender(<BrowserPanel {...props} url="https://agent.example/" reveal={1} />);
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    expect(screen.getByTestId("native-tab").dataset.id).toBe(props.paneId);
  });

  it("moves keyboard focus with arrows and Home/End using a roving tab stop", () => {
    render(<BrowserPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
    const first = screen.getByRole("tab", { name: "example.com" });
    const second = screen.getByRole("tab", { name: "New tab" });
    expect(second).toHaveFocus();
    fireEvent.keyDown(second, { key: "ArrowRight" });
    expect(first).toHaveFocus();
    expect(first.tabIndex).toBe(0);
    expect(second.tabIndex).toBe(-1);
    fireEvent.keyDown(first, { key: "End" });
    expect(second).toHaveFocus();
    fireEvent.keyDown(second, { key: "Home" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "ArrowLeft" });
    expect(second).toHaveFocus();
    expect(screen.getByRole("tabpanel")).toHaveAttribute("aria-labelledby", second.id);
  });

  it("returns focus to the remaining active tab when the selected tab closes", () => {
    render(<BrowserPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
    fireEvent.click(screen.getByRole("button", { name: "Close New tab" }));
    expect(screen.getByRole("tab", { name: "example.com" })).toHaveFocus();
  });
});
