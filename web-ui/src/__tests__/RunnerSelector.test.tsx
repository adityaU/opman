import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RunnerSelector } from "../prompt-input/RunnerSelector";

describe("RunnerSelector", () => {
  it("keeps the selected effort as a real control instead of resetting it", async () => {
    const user = userEvent.setup();
    const onEffortChange = vi.fn();

    render(
      <RunnerSelector
        currentRunner="codex"
        availableRunners={["opencode", "claude", "codex"]}
        supportedEfforts={["low", "medium", "high"]}
        effort={null}
        permission="on-request"
        disabled={false}
        onEffortChange={onEffortChange}
      />
    );

    expect(onEffortChange).not.toHaveBeenCalled();
    await user.click(screen.getByTitle("Choose runner, effort, and permissions"));
    await user.click(screen.getByRole("radio", { name: "high" }));

    expect(onEffortChange).toHaveBeenCalledWith("high");
  });

  it("shows the selected effort even when the model metadata arrives late", async () => {
    const user = userEvent.setup();
    const onEffortChange = vi.fn();

    render(
      <RunnerSelector
        currentRunner="claude"
        availableRunners={["claude"]}
        supportedEfforts={[]}
        effort="medium"
        permission="default"
        disabled={false}
        onEffortChange={onEffortChange}
      />
    );

    await user.click(screen.getByTitle("Choose runner, effort, and permissions"));
    expect(screen.getByRole("radio", { name: "medium" })).toHaveAttribute("aria-checked", "true");
  });
});
