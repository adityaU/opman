/**
 * List traversal, driven through the real keymap.
 *
 * Rendered inside a `KeymapProvider` with the live listener rather than by
 * calling the hook's internals, because half of what is being asserted is the
 * scoping: `j` moves the sidebar only while the sidebar has focus, and never
 * while a field inside it does.
 */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import { useSurfaceFocus } from "../keybindings/useSurfaceFocus";
import { useListNav } from "../keybindings/useListNav";
import type { Host } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function Listener() {
  useKeymapListener();
  useSurfaceFocus();
  return null;
}

/** A project header with two sessions under it, plus a filter field. */
function Sidebar({ open }: { readonly open: boolean }) {
  useListNav({
    surface: "sidebar",
    commands: {
      moveDown: "sidebar.moveDown",
      moveUp: "sidebar.moveUp",
      expand: "sidebar.expand",
      collapse: "sidebar.collapse",
      activate: "sidebar.open",
    },
  });

  return (
    <aside data-surface="sidebar">
      <input data-testid="filter" />
      <button data-list-item="" data-list-key="proj" data-list-depth={0} aria-expanded={open}>
        project
      </button>
      {open && (
        <>
          <button data-list-item="" data-list-key="a" data-list-depth={1}>
            session a
          </button>
          <button data-list-item="" data-list-key="b" data-list-depth={1}>
            session b
          </button>
        </>
      )}
    </aside>
  );
}

function Harness({ open = true }: { readonly open?: boolean }) {
  return (
    <KeymapProvider config={DEFAULT_CONFIG} host={HOST}>
      <Listener />
      <Sidebar open={open} />
    </KeymapProvider>
  );
}

const focused = () => document.activeElement?.textContent?.trim();

describe("list traversal", () => {
  it("steps down and up with j and k", async () => {
    render(<Harness />);
    screen.getByText("project").focus();

    await userEvent.keyboard("j");
    expect(focused()).toBe("session a");
    await userEvent.keyboard("j");
    expect(focused()).toBe("session b");
    await userEvent.keyboard("k");
    expect(focused()).toBe("session a");
  });

  it("answers to the arrows as well", async () => {
    render(<Harness />);
    screen.getByText("project").focus();

    await userEvent.keyboard("{ArrowDown}");
    expect(focused()).toBe("session a");
    await userEvent.keyboard("{ArrowUp}");
    expect(focused()).toBe("project");
  });

  it("stops at each end rather than wrapping", async () => {
    render(<Harness />);
    screen.getByText("project").focus();

    await userEvent.keyboard("k");
    expect(focused()).toBe("project");
    await userEvent.keyboard("jjj");
    expect(focused()).toBe("session b");
  });

  it("climbs to the parent row on h from a leaf", async () => {
    render(<Harness />);
    screen.getByText("session b").focus();

    await userEvent.keyboard("h");
    expect(focused()).toBe("project");
  });

  it("keeps exactly one row in the tab order", async () => {
    render(<Harness />);
    screen.getByText("project").focus();
    await userEvent.keyboard("j");

    const stops = screen
      .getAllByRole("button")
      .filter((el) => el.hasAttribute("data-list-item") && el.tabIndex === 0);
    expect(stops).toHaveLength(1);
    expect(stops[0].textContent).toBe("session a");
  });
});

describe("list scoping", () => {
  it("types into a field inside the surface instead of moving", async () => {
    render(<Harness />);
    const filter = screen.getByTestId("filter");
    filter.focus();

    await userEvent.keyboard("jk");
    expect((filter as HTMLInputElement).value).toBe("jk");
    expect(document.activeElement).toBe(filter);
  });

  it("does nothing while another surface has focus", async () => {
    render(
      <KeymapProvider config={DEFAULT_CONFIG} host={HOST}>
        <Listener />
        <Sidebar open />
        <div data-surface="chat">
          <button data-testid="elsewhere">elsewhere</button>
        </div>
      </KeymapProvider>,
    );
    screen.getByTestId("elsewhere").focus();

    await userEvent.keyboard("j");
    expect(focused()).toBe("elsewhere");
  });
});
