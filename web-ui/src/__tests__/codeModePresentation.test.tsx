import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CodeModeCard } from "../tool-call/CodeModeCard";
import { CodeModeResult } from "../tool-call/CodeModeResult";

vi.mock("../hooks/useAutoOpen", () => ({
  useAutoOpen: () => ({ shouldAutoOpen: () => true }),
}));

const shellJson = JSON.stringify({
  stdout: "created file\nready",
  stderr: "notice: using defaults",
  exit_code: null,
});

const capped = (text: string) => ({ text, bytes: text.length, truncated: false });
const callLog = {
  version: 1,
  calls: [{
    tool: "opman_tools.bash",
    input: capped(JSON.stringify({ command: "printf ready" })),
    output: capped(shellJson),
    ok: true,
    finished: true,
    durationMs: 8,
  }],
};

function expectRawOriginal(output: string) {
  const toggle = screen.getByRole("button", { name: "Show raw output" });
  expect(toggle).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(toggle);
  expect(toggle).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText(output)).toBeInTheDocument();
}

describe("code mode presentation", () => {
  it("honors the current auto-open preference and keeps script details closed", () => {
    render(<CodeModeCard part={{
      type: "tool",
      tool: "code_execute",
      state: {
        status: "completed",
        input: { code: "return 42" },
        output: "done",
      },
    }} />);

    expect(screen.getByRole("button", { name: /Execute.*Code mode/i }))
      .toHaveAttribute("aria-expanded", "true");

    expect(screen.getByText("View script").closest("details")).not.toHaveAttribute("open");
  });

  it("leaves the final output visible and last while script and call details toggle independently", () => {
    const marked = `final answer\n<opman-code-calls>${JSON.stringify(callLog)}</opman-code-calls>`;
    const { container } = render(<CodeModeCard part={{
      type: "tool",
      tool: "code_execute",
      state: {
        status: "completed",
        input: { code: "return await tools.opman_tools.bash({command: 'printf ready'})" },
        output: marked,
      },
    }} />);

    const finalOutput = screen.getByRole("region", { name: "Final code mode output" });
    const scriptSummary = screen.getByText("View script");
    const callSummary = screen.getByText("Bash");

    expect(finalOutput).toHaveTextContent("final answer");
    expect(finalOutput.parentElement?.lastElementChild).toBe(finalOutput);
    expect(scriptSummary.closest("details")).not.toHaveAttribute("open");
    expect(callSummary.closest("details")).not.toHaveAttribute("open");

    fireEvent.click(scriptSummary);
    expect(scriptSummary.closest("details")).toHaveAttribute("open");
    expect(callSummary.closest("details")).not.toHaveAttribute("open");
    expect(finalOutput).toBeVisible();

    fireEvent.click(callSummary);
    expect(callSummary.closest("details")).toHaveAttribute("open");
    expect(scriptSummary.closest("details")).toHaveAttribute("open");
    expect(finalOutput).toBeVisible();
    expect(container.querySelector(".gmc-card-body")?.lastElementChild).toBe(finalOutput);
  });
});

describe("CodeModeResult", () => {
  it("renders shell output readably and preserves the exact original in Raw", () => {
    const { container } = render(<CodeModeResult output={shellJson} />);

    expect(container.querySelector(".bash-output")).toHaveTextContent("created file");
    expect(container.querySelector(".bash-output")).toHaveTextContent("notice: using defaults");
    expectRawOriginal(shellJson);
  });

  it("keeps stderr and a nonzero exit visible and preserves the exact original in Raw", () => {
    const output = JSON.stringify({
      stdout: "partial output",
      stderr: "permission denied",
      exit_code: 7,
      timed_out: false,
    });
    const { container } = render(<CodeModeResult output={output} />);

    expect(container.querySelector(".bash-output")).toHaveTextContent("partial output");
    expect(container.querySelector(".bash-output")).toHaveTextContent("permission denied");
    expect(container.querySelector(".bash-output")).toHaveTextContent("exit code 7");
    expectRawOriginal(output);
  });

  it("keeps timeout details visible and preserves the exact original in Raw", () => {
    const output = JSON.stringify({
      stdout: "last output",
      stderr: "still waiting",
      exit_code: 124,
      timed_out: true,
    });
    const { container } = render(<CodeModeResult output={output} />);

    expect(container.querySelector(".bash-output")).toHaveTextContent("last output");
    expect(container.querySelector(".bash-output")).toHaveTextContent("still waiting");
    expect(container.querySelector(".bash-output")).toHaveTextContent("timed out");
    expectRawOriginal(output);
  });

  it("preserves every generic JSON value in the key/value presentation", () => {
    const { container } = render(
      <CodeModeResult output='{"message":"keep this JSON","count":3}' />,
    );

    expect(container.querySelectorAll(".gmc-kv-row")).toHaveLength(2);
    expect(screen.getByText("message")).toBeInTheDocument();
    expect(screen.getByText("keep this JSON")).toBeInTheDocument();
    expect(screen.getByText("count")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show raw output" })).toBeNull();
  });

  it("leaves plain text unchanged", () => {
    render(<CodeModeResult output="leave this output untouched" />);

    expect(screen.getByText("leave this output untouched")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show raw output" })).toBeNull();
  });

  it("shows an explicit message for an empty output", () => {
    render(<CodeModeResult output="" />);

    expect(screen.getByText("(no output returned)")).toBeInTheDocument();
  });

  it("shows omitted-byte warnings without losing the received raw response", () => {
    const output = JSON.stringify({ stdout: "partial", stderr: "", exit_code: 0,
      stdout_omitted_bytes: 2048, stderr_omitted_bytes: 12 });
    render(<CodeModeResult output={output} />);
    expect(screen.getByRole("region", { name: "Final code mode output" }))
      .toHaveTextContent("stdout truncated: 2048 bytes omitted");
    expect(screen.getByRole("region", { name: "Final code mode output" }))
      .toHaveTextContent("stderr truncated: 12 bytes omitted");
    expectRawOriginal(output);
  });
});
