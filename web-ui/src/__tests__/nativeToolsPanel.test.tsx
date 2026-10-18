/**
 * Settings → Agents: the native-tools switches under an enabled agent.
 */
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { NativeTool, NativeToolsInfo } from "../api/acp";
import { NativeToolsPanel } from "../settings-page/acp/NativeToolsPanel";
import {
  afterMasterClick,
  afterToolClick,
  applySetting,
  masterState,
} from "../settings-page/acp/nativeToolsModel";

afterEach(cleanup);

const tool = (id: string, enabled: boolean): NativeTool => ({
  id,
  label: id.toUpperCase(),
  description: `${id} does a thing`,
  enabled,
});

const info = (...tools: NativeTool[]): NativeToolsInfo => ({
  setting: "all",
  note: "Sent when a session starts.",
  tools,
});

function show(value: NativeToolsInfo | null, onChange = vi.fn()) {
  render(<NativeToolsPanel agentName="Claude" info={value} disabled={false} onChange={onChange} />);
  return onChange;
}

describe("nativeToolsModel", () => {
  it("names the three master states", () => {
    expect(masterState([tool("a", true), tool("b", true)])).toBe("all");
    expect(masterState([tool("a", false), tool("b", false)])).toBe("none");
    expect(masterState([tool("a", true), tool("b", false)])).toBe("some");
  });

  it("master turns everything on unless it already is", () => {
    expect(afterMasterClick([tool("a", true), tool("b", true)])).toBe("off");
    expect(afterMasterClick([tool("a", true), tool("b", false)])).toBe("all");
    expect(afterMasterClick([tool("a", false), tool("b", false)])).toBe("all");
  });

  it("a tool click lists exactly the tools that end up off", () => {
    const tools = [tool("a", true), tool("b", true), tool("c", true)];
    expect(afterToolClick(tools, "b")).toEqual({ disabled: ["b"] });
    expect(afterToolClick([tool("a", true), tool("b", false)], "b")).toBe("all");
    expect(afterToolClick([tool("a", false), tool("b", true)], "b")).toBe("off");
  });

  it("applies a setting to the tool list for an optimistic update", () => {
    const base = info(tool("a", true), tool("b", true));
    expect(applySetting(base, { disabled: ["a"] }).tools.map((t) => t.enabled)).toEqual([false, true]);
    expect(applySetting(base, "off").tools.every((t) => !t.enabled)).toBe(true);
  });
});

describe("NativeToolsPanel", () => {
  it("says so, with no switches, when the agent cannot be controlled", () => {
    show(null);
    expect(screen.getByText(/can't be controlled for this agent/i)).toBeTruthy();
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("shows a master switch and a count, with the tools folded away", () => {
    show(info(tool("bash", true), tool("read", false)));
    expect(screen.getByText("1 of 2 on")).toBeTruthy();
    expect(screen.getByText(/applies to new sessions/i)).toBeTruthy();
    expect(screen.getByRole("checkbox")).toBeTruthy();
    expect(screen.queryAllByRole("switch")).toHaveLength(0);
  });

  it("shows the master as mixed when only some tools are on", () => {
    show(info(tool("bash", true), tool("read", false)));
    const master = screen.getByRole("checkbox", { name: /all native tools/i });
    expect(master.getAttribute("data-mixed")).toBe("true");
    expect(master.getAttribute("aria-checked")).toBe("mixed");
    expect(screen.getByText("Mixed")).toBeTruthy();
  });

  it("master click turns everything on when mixed, and off when all on", () => {
    const mixed = show(info(tool("bash", true), tool("read", false)));
    fireEvent.click(screen.getByRole("checkbox", { name: /all native tools/i }));
    expect(mixed).toHaveBeenCalledWith("all");
    cleanup();
    const allOn = show(info(tool("bash", true), tool("read", true)));
    fireEvent.click(screen.getByRole("checkbox", { name: /all native tools/i }));
    expect(allOn).toHaveBeenCalledWith("off");
  });

  it("opens the list and flips one tool", () => {
    const onChange = show(info(tool("bash", true), tool("read", true)));
    fireEvent.click(screen.getByRole("button", { name: /native tools/i }));
    expect(screen.getAllByRole("switch")).toHaveLength(2);
    fireEvent.click(screen.getByRole("switch", { name: "BASH for Claude" }));
    expect(onChange).toHaveBeenCalledWith({ disabled: ["bash"] });
  });

  it("disables every switch while a write is in flight", () => {
    render(
      <NativeToolsPanel
        agentName="Claude"
        info={info(tool("bash", true))}
        disabled
        onChange={vi.fn()}
      />,
    );
    expect(
      (screen.getByRole("checkbox", { name: /all native tools/i }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
