import { describe, expect, it } from "vitest";
import {
  agentManagerAction,
  isAgentManagerTool,
  parseAgentOutput,
} from "../tool-call/agent-manager";
import {
  agentRows,
  openableSessionId,
  runnerOptions,
} from "../tool-call/agent-manager/model";

describe("agent manager tool card helpers", () => {
  it("recognizes MCP-prefixed manager tools", () => {
    expect(agentManagerAction("mcp__agent-manager__agent_start")).toBe("start");
    expect(agentManagerAction("agent-manager_agent_send")).toBe("send");
    expect(agentManagerAction("agent_progress")).toBe("progress");
    expect(isAgentManagerTool("mcp__agent-manager__agent_progress")).toBe(true);
  });

  it("recognizes every tool the manager exposes", () => {
    expect(agentManagerAction("mcp__agent-manager__agent_list")).toBe("list");
    expect(agentManagerAction("mcp__agent-manager__agent_wait")).toBe("wait");
    expect(agentManagerAction("mcp__agent-manager__agent_abort")).toBe("abort");
    expect(agentManagerAction("mcp__agent-manager__agent_runner_options")).toBe("options");
  });

  it("does not claim unrelated tools", () => {
    expect(agentManagerAction("task")).toBe("other");
    expect(isAgentManagerTool("send_message")).toBe(false);
  });

  it("parses the pretty JSON returned by the MCP bridge", () => {
    expect(parseAgentOutput('{\n  "delivery": "queued",\n  "queued_messages": 2\n}')).toEqual({
      delivery: "queued",
      queued_messages: 2,
    });
  });

  it("reads the agent_list rows, defaulting a missing title to the id", () => {
    const rows = agentRows({
      agents: [
        { agent_id: "ses_abcdefghijklmnop", title: "Refactor", runner: "claude", busy: true, queued_messages: 2 },
        { agent_id: "ses_two" },
      ],
    });
    expect(rows[0]).toEqual({
      id: "ses_abcdefghijklmnop",
      title: "Refactor",
      runner: "claude",
      busy: true,
      queued: 2,
    });
    expect(rows[1]).toEqual({ id: "ses_two", title: "ses_two", runner: "", busy: false, queued: 0 });
  });

  it("flattens the runner catalogue and its caps", () => {
    const options = runnerOptions({
      runner: "claude",
      models: [{ provider: "anthropic", id: "claude-opus-5", efforts: ["low", "high"] }],
      efforts: ["low", "high"],
      total_models: 42,
      omitted_models: 3,
      connected: ["anthropic"],
      permission_modes: ["default", "plan"],
      agents: ["Explore"],
    });
    expect(options.models[0].name).toBe("claude-opus-5");
    expect(options.total).toBe(42);
    expect(options.omitted).toBe(3);
    expect(options.permissionModes).toEqual(["default", "plan"]);
  });

  it("finds the session a card can open, and refuses 'parent'", () => {
    expect(openableSessionId("start", {}, { session_id: "ses_1" })).toBe("ses_1");
    expect(openableSessionId("wait", { agent_id: "ses_2" }, {})).toBe("ses_2");
    expect(openableSessionId("progress", { agent_id: "parent" }, {})).toBe("");
    expect(openableSessionId("list", {}, {})).toBe("");
  });
});
