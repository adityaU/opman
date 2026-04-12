/**
 * Unit tests for QuestionDock and QuestionCard components.
 */
import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QuestionDock } from "../QuestionDock";
import type { QuestionRequest } from "../types";

type OnReply = (requestId: string, answers: string[][]) => void;
type OnDismiss = (requestId: string) => void;

// ── Helpers ─────────────────────────────────────────────
function makeQuestion(overrides: Partial<QuestionRequest> = {}): QuestionRequest {
  return {
    id: "q1",
    sessionID: "s1",
    title: "Test Question",
    time: Date.now(),
    questions: [
      { text: "Pick one", type: "select", options: ["A", "B", "C"] },
    ],
    ...overrides,
  };
}

describe("QuestionDock", () => {
  let onReply: Mock<OnReply>;
  let onDismiss: Mock<OnDismiss>;

  beforeEach(() => {
    onReply = vi.fn<OnReply>();
    onDismiss = vi.fn<OnDismiss>();
  });

  it("renders with role='region'", () => {
    const { container } = render(
      <QuestionDock questions={[makeQuestion()]} onReply={onReply} onDismiss={onDismiss} />
    );
    expect(container.querySelector('[role="region"]')).toBeTruthy();
  });

  it("renders one card per question", () => {
    const qs = [
      makeQuestion({ id: "q1", title: "First" }),
      makeQuestion({ id: "q2", title: "Second" }),
    ];
    render(<QuestionDock questions={qs} onReply={onReply} onDismiss={onDismiss} />);
    expect(screen.getAllByText("First").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Second").length).toBeGreaterThanOrEqual(1);
  });

  it("renders select options as buttons", () => {
    render(
      <QuestionDock
        questions={[makeQuestion()]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );
    expect(screen.getByText("A")).toBeTruthy();
    expect(screen.getByText("B")).toBeTruthy();
    expect(screen.getByText("C")).toBeTruthy();
  });

  it("clicking a select option marks it selected", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock questions={[makeQuestion()]} onReply={onReply} onDismiss={onDismiss} />
    );

    const spanA = screen.getByText("A");
    const btnA = spanA.closest("button")!;
    await user.click(btnA);
    expect(btnA.getAttribute("aria-selected")).toBe("true");
  });

  it("single-select replaces selection on second click", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Pick one", type: "select", options: ["A", "B"], multiple: false }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );

    await user.click(screen.getByText("A"));
    await user.click(screen.getByText("B"));
    expect(screen.getByText("A").closest("button")!.getAttribute("aria-selected")).toBe("false");
    expect(screen.getByText("B").closest("button")!.getAttribute("aria-selected")).toBe("true");
  });

  it("multi-select allows multiple selections", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Pick many", type: "select", options: ["A", "B", "C"], multiple: true }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );

    await user.click(screen.getByText("A"));
    await user.click(screen.getByText("C"));
    expect(screen.getByText("A").closest("button")!.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("C").closest("button")!.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("B").closest("button")!.getAttribute("aria-selected")).toBe("false");
  });

  it("multi-select deselects on second click", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Pick many", type: "select", options: ["A", "B"], multiple: true }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );

    await user.click(screen.getByText("A"));
    expect(screen.getByText("A").closest("button")!.getAttribute("aria-selected")).toBe("true");
    await user.click(screen.getByText("A"));
    expect(screen.getByText("A").closest("button")!.getAttribute("aria-selected")).toBe("false");
  });

  it("renders confirm question with Yes/No buttons", () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Are you sure?", type: "confirm" }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );
    expect(screen.getByText("Yes")).toBeTruthy();
    expect(screen.getByText("No")).toBeTruthy();
  });

  it("clicking Yes on confirm sets answer to ['yes']", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Are you sure?", type: "confirm" }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );

    await user.click(screen.getByText("Yes"));
    // The Yes button should be marked as pressed
    expect(screen.getByText("Yes").getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("No").getAttribute("aria-pressed")).toBe("false");
  });

  it("renders text input for text-type questions", () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Enter name", type: "text" }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );
    expect(screen.getByPlaceholderText("Type your answer...")).toBeTruthy();
  });

  it("submit button calls onReply with question id and answers", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          id: "q42",
          questions: [{ text: "Pick one", type: "select", options: ["A", "B"] }],
        })]}
        onReply={onReply}
        onDismiss={onDismiss}
      />
    );

    await user.click(screen.getByText("B"));
    await user.click(screen.getByLabelText("Submit answers"));

    expect(onReply).toHaveBeenCalledWith("q42", [["B"]]);
  });

  it("Enter key on a non-text element submits the form", async () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          id: "q99",
          questions: [{ text: "Pick", type: "select", options: ["X"] }],
        })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    // Click option first
    fireEvent.click(screen.getByText("X"));

    // Press Enter on the option button
    fireEvent.keyDown(screen.getByText("X"), { key: "Enter" });

    expect(onReply).toHaveBeenCalledWith("q99", [["X"]]);
  });

  it("Enter key does NOT submit when no answer is selected", () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          id: "q100",
          questions: [{ text: "Pick", type: "select", options: ["X", "Y"] }],
        })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    // Press Enter without selecting anything
    fireEvent.keyDown(screen.getByText("X"), { key: "Enter" });

    expect(onReply).not.toHaveBeenCalled();
  });

  it("Escape key dismisses the question", () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          id: "q101",
          questions: [{ text: "Pick", type: "select", options: ["X"] }],
        })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    // Press Escape on an option button
    fireEvent.keyDown(screen.getByText("X"), { key: "Escape" });

    expect(onDismiss).toHaveBeenCalledWith("q101");
    expect(onReply).not.toHaveBeenCalled();
  });

  it("dismiss button calls onDismiss with question id", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({ id: "q102" })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    await user.click(screen.getByLabelText("Dismiss question"));

    expect(onDismiss).toHaveBeenCalledWith("q102");
    expect(onReply).not.toHaveBeenCalled();
  });

  it("submit button is disabled when no answer is selected", () => {
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Pick", type: "select", options: ["A"] }],
        })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    const submitBtn = screen.getByLabelText("Submit answers");
    expect(submitBtn.hasAttribute("disabled")).toBe(true);
  });

  it("submit button is enabled after selecting an answer", async () => {
    const user = userEvent.setup();
    render(
      <QuestionDock
        questions={[makeQuestion({
          questions: [{ text: "Pick", type: "select", options: ["A"] }],
        })]}
        onReply={onReply} onDismiss={onDismiss} />
    );

    await user.click(screen.getByText("A"));

    const submitBtn = screen.getByLabelText("Submit answers");
    expect(submitBtn.hasAttribute("disabled")).toBe(false);
  });
});
