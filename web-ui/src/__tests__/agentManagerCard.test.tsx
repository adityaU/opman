import React from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  AgentManagerToolCard,
  AgentSessionOpenProvider,
  type AgentSessionOpener,
} from "../tool-call/agent-manager";
import type { MessagePart } from "../types";

/** A completed tool call, as the transcript hands it to the card. */
function part(tool: string, input: unknown, output: unknown): MessagePart {
  return {
    type: "tool",
    tool,
    state: { status: "completed", input, output: JSON.stringify(output) },
  } as unknown as MessagePart;
}

function opener(overrides: Partial<AgentSessionOpener> = {}): AgentSessionOpener {
  return { canOpen: () => true, open: () => {}, ...overrides };
}

/**
 * Render expanded. A completed card is collapsed unless the user has opted its
 * category into auto-open, and every assertion here is about the body.
 */
function draw(node: React.ReactElement, api: AgentSessionOpener = opener()) {
  const result = render(<AgentSessionOpenProvider value={api}>{node}</AgentSessionOpenProvider>);
  fireEvent.click(screen.getByRole("button", { expanded: false }));
  return result;
}

describe("AgentManagerToolCard", () => {
  it("offers the started session to a pane, by label", async () => {
    const open = vi.fn();
    draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_start",
          { title: "Refactor pass", model: "claude-opus-5", effort: "high" },
          { session_id: "ses_started", runner: "claude", delivery: "immediate" },
        )}
      />,
      opener({ open }),
    );
    fireEvent.click(screen.getAllByRole("button", { name: /open session/i })[0]);
    expect(open).toHaveBeenCalledWith("ses_started", "Refactor pass");
  });

  it("draws no link when no open project holds the session", () => {
    draw(
      <AgentManagerToolCard
        part={part("mcp__agent-manager__agent_start", {}, { session_id: "ses_gone" })}
      />,
      opener({ canOpen: () => false }),
    );
    expect(screen.queryByRole("button", { name: /open session/i })).toBeNull();
  });

  it("renders one openable row per listed agent", () => {
    draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_list",
          {},
          {
            count: 2,
            agents: [
              { agent_id: "ses_a", title: "Alpha", runner: "claude", busy: true, queued_messages: 1 },
              { agent_id: "ses_b", title: "Beta", runner: "codex", busy: false, queued_messages: 0 },
            ],
          },
        )}
      />,
    );
    expect(screen.getByText("Alpha")).toBeTruthy();
    expect(screen.getByText("1 queued")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: /open session/i })).toHaveLength(2);
  });

  it("distinguishes a finished wait from a timed-out one", () => {
    const { unmount } = draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_wait",
          { agent_id: "ses_w" },
          { agent_id: "ses_w", timed_out: false, busy: false, reply: "all done" },
        )}
      />,
    );
    expect(screen.getByText("Turn finished")).toBeTruthy();
    expect(screen.getByText("all done")).toBeTruthy();
    unmount();

    draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_wait",
          { agent_id: "ses_w", timeout: 300 },
          { agent_id: "ses_w", timed_out: true, busy: true, reply: "" },
        )}
      />,
    );
    expect(screen.getByText(/Timed out/)).toBeTruthy();
    expect(screen.getByText("limit 300s")).toBeTruthy();
  });

  it("reports an abort as cancelled without losing the session", () => {
    draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_abort",
          { agent_id: "ses_x" },
          { agent_id: "ses_x", aborted: true },
        )}
      />,
    );
    expect(screen.getByText(/Turn cancelled/)).toBeTruthy();
  });

  it("summarises the runner catalogue and says what it left out", () => {
    draw(
      <AgentManagerToolCard
        part={part(
          "mcp__agent-manager__agent_runner_options",
          { runner: "claude" },
          {
            runner: "claude",
            models: [{ provider: "anthropic", id: "claude-opus-5", name: "Opus 5", efforts: ["high"] }],
            efforts: ["low", "high"],
            total_models: 30,
            omitted_models: 4,
            connected: ["anthropic"],
            permission_modes: ["default", "plan"],
            agents: ["Explore"],
          },
        )}
      />,
    );
    expect(screen.getByText("Opus 5")).toBeTruthy();
    expect(screen.getByText("plan")).toBeTruthy();
    expect(screen.getByText(/4 more/)).toBeTruthy();
  });
});
