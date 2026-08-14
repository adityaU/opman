/**
 * The slash list owns its keys.
 *
 * The bug this pins: the popover listened on `document` in capture and let the event
 * through, so one Enter both picked the highlighted command *and* let the composer
 * submit the half-typed name that filtered it — `/ag` reached the runner as a plain
 * message next to the `/agent` the user actually chose.
 */
import React, { useEffect, useState } from "react";
import { render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SlashCommandPopover } from "../SlashCommandPopover";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useCommands } from "../keybindings/useCommand";

vi.mock("../api", () => ({ fetchCommands: vi.fn(async () => []) }));

/**
 * A command is only offered when a mounted surface answers for it — and registration
 * happens in an effect, so the list is mounted a tick later, as it is in the app.
 */
function WithAgentPicker({ children }: { children: React.ReactNode }) {
  useCommands({ "engine.agent": () => {} });
  const [ready, setReady] = useState(false);
  useEffect(() => setReady(true), []);
  return ready ? <>{children}</> : null;
}

function open(onSelect: () => void) {
  render(
    <KeymapProvider>
      <WithAgentPicker>
        <SlashCommandPopover filter="ag" onSelect={onSelect} onClose={() => {}} sessionId="ses_1" />
      </WithAgentPicker>
    </KeymapProvider>,
  );
}

afterEach(cleanup);

describe("the slash popover's keys", () => {
  it("keeps Enter from also reaching the composer", async () => {
    const onSelect = vi.fn();
    const composer = vi.fn();
    document.addEventListener("keydown", composer);
    open(onSelect);
    await screen.findByRole("listbox");

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(composer).not.toHaveBeenCalled();
    document.removeEventListener("keydown", composer);
  });

  it("leaves keys it does not handle alone", async () => {
    const composer = vi.fn();
    document.addEventListener("keydown", composer);
    open(vi.fn());
    await screen.findByRole("listbox");

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));

    expect(composer).toHaveBeenCalledTimes(1);
    document.removeEventListener("keydown", composer);
  });
});
