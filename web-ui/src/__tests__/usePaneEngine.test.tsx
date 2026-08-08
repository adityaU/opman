/**
 * A pane's engine. The two things worth pinning are that touching one field
 * does not silently strand the pane on a half-copied engine, and that the
 * runner reaches the wire exactly once per switch — naming it on every send is
 * what forks a conversation into a handoff.
 */
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { usePaneEngine } from "../workspace/widgets/usePaneEngine";
import {
  WorkspaceChatProvider,
  type WorkspaceChatServices,
} from "../workspace/widgets/WorkspaceChatContext";
import type { PaneEngine } from "../workspace/types";

const SHELL: PaneEngine = {
  runner: "opencode",
  model: { providerID: "zen", modelID: "big-pickle" },
  agent: "build",
  effort: null,
  permission: "default",
};

function harness(own: PaneEngine | null, setEngine = vi.fn()) {
  const services = { defaultEngine: SHELL, setEngine } as unknown as WorkspaceChatServices;
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <WorkspaceChatProvider value={services}>{children}</WorkspaceChatProvider>
  );
  return { ...renderHook(() => usePaneEngine("p1", own), { wrapper }), setEngine };
}

describe("usePaneEngine", () => {
  it("follows the shell until the pane has an engine of its own", () => {
    expect(harness(null).result.current.engine).toEqual(SHELL);
  });

  it("prefers the pane's own engine over the shell's", () => {
    const own: PaneEngine = { ...SHELL, runner: "codex", agent: "codex-default" };
    expect(harness(own).result.current.engine).toEqual(own);
  });

  it("materialises from the shell's engine on the first change", () => {
    const { result, setEngine } = harness(null);
    act(() => result.current.setAgent("plan"));
    // Not just `{ agent: "plan" }` — the pane would otherwise be left with no
    // runner and no model the moment the shell's changed underneath it.
    expect(setEngine).toHaveBeenCalledWith("p1", { ...SHELL, agent: "plan" });
  });

  it("clears the runner-scoped fields when the runner changes", () => {
    const { result, setEngine } = harness(null);
    act(() => result.current.setRunner("codex"));
    expect(setEngine).toHaveBeenCalledWith("p1", {
      runner: "codex",
      model: null,
      agent: "",
      effort: null,
      permission: "default",
    });
  });

  it("arms the runner switch once and disarms it after the send", () => {
    const { result } = harness(null);
    expect(result.current.switchRunner).toBe(false);

    act(() => result.current.setRunner("codex"));
    expect(result.current.switchRunner).toBe(true);

    act(() => result.current.runnerSent());
    expect(result.current.switchRunner).toBe(false);
  });

  it("does not arm a switch for the runner the pane is already on", () => {
    const { result, setEngine } = harness(null);
    act(() => result.current.setRunner(SHELL.runner));
    expect(setEngine).not.toHaveBeenCalled();
    expect(result.current.switchRunner).toBe(false);
  });

  it("keeps the rest of the engine when one field changes", () => {
    const own: PaneEngine = { ...SHELL, runner: "codex", model: null };
    const { result, setEngine } = harness(own);
    act(() => result.current.setEffort("high"));
    expect(setEngine).toHaveBeenCalledWith("p1", { ...own, effort: "high" });
    act(() => result.current.setModel("gpt-5", "openai"));
    expect(setEngine).toHaveBeenLastCalledWith("p1", {
      ...own,
      model: { providerID: "openai", modelID: "gpt-5" },
    });
  });
});
