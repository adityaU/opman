/**
 * The composer's dateline: the session name it shows, and renaming in place.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SessionLine } from "../prompt-input/SessionLine";

const renameSession = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("../api", () => ({ renameSession }));

describe("SessionLine", () => {
  beforeEach(() => renameSession.mockClear());

  it("shows the session name", () => {
    render(<SessionLine title="Fix the dock" sessionId="s1" busy={false} />);
    expect(screen.getByText("Fix the dock")).toBeTruthy();
  });

  it("falls back to New session and stays uneditable without a session", () => {
    render(<SessionLine title={null} sessionId={null} busy={false} />);
    expect(screen.getByText("New session")).toBeTruthy();
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(true);
  });

  it("marks the dot busy while a turn runs", () => {
    const { container } = render(<SessionLine title="t" sessionId="s1" busy />);
    expect(container.querySelector('.composer-session-mark[data-state="busy"]')).toBeTruthy();
  });

  it("renames on Enter and shows the new name before the server echoes it", async () => {
    const user = userEvent.setup();
    render(<SessionLine title="Old name" sessionId="s1" busy={false} />);

    await user.click(screen.getByText("Old name"));
    const field = screen.getByLabelText("Session name") as HTMLInputElement;
    await user.clear(field);
    await user.type(field, "New name{Enter}");

    expect(renameSession).toHaveBeenCalledWith("s1", "New name");
    await waitFor(() => expect(screen.getByText("New name")).toBeTruthy());
  });

  it("Escape cancels without calling the API", async () => {
    const user = userEvent.setup();
    render(<SessionLine title="Old name" sessionId="s1" busy={false} />);

    await user.click(screen.getByText("Old name"));
    await user.type(screen.getByLabelText("Session name"), "scratch");
    // The shell claims Escape at capture phase, which is where the row listens.
    fireEvent.keyDown(window, { key: "Escape" });

    expect(renameSession).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByText("Old name")).toBeTruthy());
  });

  it("an unchanged name is not sent", async () => {
    const user = userEvent.setup();
    render(<SessionLine title="Same" sessionId="s1" busy={false} />);

    await user.click(screen.getByText("Same"));
    await user.type(screen.getByLabelText("Session name"), "{Enter}");

    expect(renameSession).not.toHaveBeenCalled();
  });

  it("shows the runner's progress on the same line", () => {
    const { container } = render(
      <SessionLine title="t" sessionId="s1" busy progressText="Bash · cargo build" />,
    );
    expect(screen.getByText("Bash · cargo build")).toBeTruthy();
    expect(container.querySelectorAll(".composer-session > *").length).toBe(3);
  });
});
