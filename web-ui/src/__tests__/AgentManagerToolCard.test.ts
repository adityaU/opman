import { describe, expect, it } from "vitest";
import {
  agentManagerAction,
  isAgentManagerTool,
  parseAgentOutput,
} from "../tool-call/AgentManagerToolCard";

describe("agent manager tool card helpers", () => {
  it("recognizes MCP-prefixed manager tools", () => {
    expect(agentManagerAction("mcp__agent-manager__agent_start")).toBe("start");
    expect(agentManagerAction("agent-manager_agent_send")).toBe("send");
    expect(agentManagerAction("agent_progress")).toBe("progress");
    expect(isAgentManagerTool("mcp__agent-manager__agent_progress")).toBe(true);
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
});
