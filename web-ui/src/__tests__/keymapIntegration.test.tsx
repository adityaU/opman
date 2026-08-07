import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useCommand, useCommands, useWhenContext } from "../keybindings/useCommand";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import type { Host, Mode } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function Listener() {
  const { pending } = useKeymapListener();
  return <div data-testid="pending">{pending ? pending.steps.length : 0}</div>;
}

interface HarnessProps {
  readonly onCommand: Record<string, () => void>;
  readonly context?: Record<string, string | boolean>;
  readonly mode?: Mode;
}

function Harness({ onCommand, context = {}, mode = "normal" }: HarnessProps) {
  return (
    <KeymapProvider config={{ ...DEFAULT_CONFIG, mode }} host={HOST}>
      <Listener />
      <Surface onCommand={onCommand} context={context} />
    </KeymapProvider>
  );
}

function Surface({
  onCommand,
  context,
}: {
  onCommand: Record<string, () => void>;
  context: Record<string, string | boolean>;
}) {
  useCommands(onCommand);
  useWhenContext(context);
  return <textarea data-testid="composer" />;
}

describe("keymap dispatch", () => {
  it("runs a command from a single chord", async () => {
    const toggleSidebar = vi.fn();
    render(<Harness onCommand={{ "layout.toggleSidebar": toggleSidebar }} />);

    await userEvent.keyboard("{Control>}b{/Control}");
    expect(toggleSidebar).toHaveBeenCalledTimes(1);
  });

  it("waits for the second step, then runs", async () => {
    const compact = vi.fn();
    render(
      <Harness onCommand={{ "chat.compact": compact }} context={{ sessionActive: true }} />,
    );

    await userEvent.keyboard("{Control>}k{/Control}");
    expect(screen.getByTestId("pending").textContent).toBe("1");
    expect(compact).not.toHaveBeenCalled();

    await userEvent.keyboard("{Control>}c{/Control}");
    expect(compact).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("pending").textContent).toBe("0");
  });

  it("abandons a chord when the second step is unbound", async () => {
    const compact = vi.fn();
    render(
      <Harness onCommand={{ "chat.compact": compact }} context={{ sessionActive: true }} />,
    );

    await userEvent.keyboard("{Control>}k{/Control}");
    await userEvent.keyboard("{Control>}9{/Control}");

    expect(compact).not.toHaveBeenCalled();
    expect(screen.getByTestId("pending").textContent).toBe("0");
  });

  it("does not fire a command whose when clause is false", async () => {
    const compact = vi.fn();
    render(<Harness onCommand={{ "chat.compact": compact }} context={{ sessionActive: false }} />);

    await userEvent.keyboard("{Control>}k{/Control}");
    await userEvent.keyboard("{Control>}c{/Control}");
    expect(compact).not.toHaveBeenCalled();
  });

  it("times out a pending chord", async () => {
    vi.useFakeTimers();
    try {
      render(<Harness onCommand={{}} />);
      // userEvent needs real timers, so the key is dispatched directly here.
      act(() => {
        document.body.dispatchEvent(
          new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }),
        );
      });
      expect(screen.getByTestId("pending").textContent).toBe("1");

      act(() => {
        vi.advanceTimersByTime(DEFAULT_CONFIG.chordTimeoutMs + 10);
      });
      expect(screen.getByTestId("pending").textContent).toBe("0");
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("insert guard", () => {
  it("lets a bare vim key type inside a textarea", async () => {
    const newFile = vi.fn();
    render(
      <Harness
        mode="vim"
        onCommand={{ "explorer.newFile": newFile }}
        context={{ focus: "explorer" }}
      />,
    );

    const composer = screen.getByTestId("composer");
    await userEvent.click(composer);
    await userEvent.keyboard("a");

    expect(newFile).not.toHaveBeenCalled();
    expect(composer).toHaveValue("a");
  });

  it("still dispatches a modifier chord inside a textarea", async () => {
    const toggleSidebar = vi.fn();
    render(<Harness mode="vim" onCommand={{ "layout.toggleSidebar": toggleSidebar }} />);

    await userEvent.click(screen.getByTestId("composer"));
    await userEvent.keyboard("{Control>}b{/Control}");
    expect(toggleSidebar).toHaveBeenCalledTimes(1);
  });

  it("dispatches a bare vim key when focus is not in a text field", async () => {
    const newFile = vi.fn();
    render(
      <Harness
        mode="vim"
        onCommand={{ "explorer.newFile": newFile }}
        context={{ focus: "explorer" }}
      />,
    );

    await userEvent.keyboard("a");
    expect(newFile).toHaveBeenCalledTimes(1);
  });
});

describe("vim leader", () => {
  it("runs a leader chord and shows the pending steps", async () => {
    const toggleGit = vi.fn();
    render(<Harness mode="vim" onCommand={{ "layout.toggleGit": toggleGit }} />);

    await userEvent.keyboard(" ");
    expect(screen.getByTestId("pending").textContent).toBe("1");
    await userEvent.keyboard("g");
    expect(screen.getByTestId("pending").textContent).toBe("2");
    await userEvent.keyboard("g");

    expect(toggleGit).toHaveBeenCalledTimes(1);
  });

  it("is inert in normal mode", async () => {
    const toggleGit = vi.fn();
    render(<Harness onCommand={{ "layout.toggleGit": toggleGit }} />);

    await userEvent.keyboard(" gg");
    expect(toggleGit).not.toHaveBeenCalled();
  });
});

describe("command registry", () => {
  it("ignores a chord with no registered handler", async () => {
    render(<Harness onCommand={{}} />);
    await userEvent.keyboard("{Control>}b{/Control}");
    expect(screen.getByTestId("pending").textContent).toBe("0");
  });

  it("clears published context on unmount", async () => {
    const newFile = vi.fn();
    function Wrapper({ show }: { show: boolean }) {
      return (
        <KeymapProvider config={{ ...DEFAULT_CONFIG, mode: "vim" }} host={HOST}>
          <Listener />
          {show ? <Surface onCommand={{ "explorer.newFile": newFile }} context={{ focus: "explorer" }} /> : null}
        </KeymapProvider>
      );
    }

    const { rerender } = render(<Wrapper show />);
    await userEvent.keyboard("a");
    expect(newFile).toHaveBeenCalledTimes(1);

    rerender(<Wrapper show={false} />);
    await userEvent.keyboard("a");
    expect(newFile).toHaveBeenCalledTimes(1);
  });
});

describe("useCommand", () => {
  it("registers a single command", async () => {
    const run = vi.fn();
    function One() {
      useCommand("layout.toggleSidebar", run);
      return null;
    }
    render(
      <KeymapProvider config={DEFAULT_CONFIG} host={HOST}>
        <Listener />
        <One />
      </KeymapProvider>,
    );

    await userEvent.keyboard("{Control>}b{/Control}");
    expect(run).toHaveBeenCalledTimes(1);
  });
});
