/**
 * Unit tests for PermissionDock and PermissionCard components.
 */
import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PermissionDock } from "../PermissionDock";
import type { PermissionRequest } from "../types";

type OnReply = (requestId: string, reply: "once" | "always" | "reject") => void;

// ── Helpers ─────────────────────────────────────────────
function makePerm(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    id: "perm1",
    sessionID: "s1",
    toolName: "bash",
    time: Date.now(),
    ...overrides,
  };
}

describe("PermissionDock", () => {
  let onReply: Mock<OnReply>;

  beforeEach(() => {
    onReply = vi.fn<OnReply>();
  });

  it("renders with role='alertdialog'", () => {
    const { container } = render(
      <PermissionDock permissions={[makePerm()]} onReply={onReply} />
    );
    expect(container.querySelector('[role="alertdialog"]')).toBeTruthy();
  });

  it("renders one card per permission", () => {
    render(
      <PermissionDock
        permissions={[
          makePerm({ id: "p1", toolName: "bash" }),
          makePerm({ id: "p2", toolName: "write" }),
        ]}
        onReply={onReply}
      />
    );
    expect(screen.getAllByText("bash").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("write").length).toBeGreaterThanOrEqual(1);
  });

  it("renders tool name", () => {
    render(
      <PermissionDock permissions={[makePerm({ toolName: "rm -rf" })]} onReply={onReply} />
    );
    expect(screen.getByText("rm -rf")).toBeTruthy();
  });

  it("renders description when provided", () => {
    render(
      <PermissionDock
        permissions={[makePerm({ description: "Delete all files" })]}
        onReply={onReply}
      />
    );
    expect(screen.getByText("Delete all files")).toBeTruthy();
  });

  it("does not render description when absent", () => {
    render(
      <PermissionDock permissions={[makePerm()]} onReply={onReply} />
    );
    // No .permission-desc element
    const { container } = render(
      <PermissionDock permissions={[makePerm()]} onReply={onReply} />
    );
    expect(container.querySelector(".permission-desc")).toBeNull();
  });

  it("renders structured details instead of raw JSON when metadata is provided", () => {
    render(
      <PermissionDock
        permissions={[makePerm({ metadata: { command: "ls -la", cwd: "/workspace" } })]}
        onReply={onReply}
      />
    );
    expect(screen.getByText("ls -la")).toBeTruthy();
    expect(screen.getByText("/workspace")).toBeTruthy();
    expect(document.querySelector(".permission-args")).toBeNull();
  });

  it("renders Codex approval metadata as readable options", () => {
    render(
      <PermissionDock
        permissions={[makePerm({ metadata: {
          availableDecisions: ["accept", { acceptWithExecpolicyAmendment: { execpolicy_amendment: ["npm test"] } }],
          command: "npm test",
        } })]}
        onReply={onReply}
      />
    );
    expect(screen.getByText("Allow once")).toBeTruthy();
    expect(screen.getByText("Allow with policy update")).toBeTruthy();
    expect(screen.getByText("npm test")).toBeTruthy();
  });

  it("does not render details when metadata is empty", () => {
    const { container } = render(
      <PermissionDock permissions={[makePerm({ metadata: {} })]} onReply={onReply} />
    );
    expect(container.querySelector(".permission-details")).toBeNull();
  });

  // ── Button clicks ────────────────────────────────────
  it("Allow Once button calls onReply with 'once'", async () => {
    const user = userEvent.setup();
    render(
      <PermissionDock permissions={[makePerm({ id: "p1" })]} onReply={onReply} />
    );
    await user.click(screen.getByLabelText("Allow once"));
    expect(onReply).toHaveBeenCalledWith("p1", "once");
  });

  it("Always Allow button calls onReply with 'always'", async () => {
    const user = userEvent.setup();
    render(
      <PermissionDock permissions={[makePerm({ id: "p1" })]} onReply={onReply} />
    );
    await user.click(screen.getByLabelText("Always allow"));
    expect(onReply).toHaveBeenCalledWith("p1", "always");
  });

  it("Reject button calls onReply with 'reject'", async () => {
    const user = userEvent.setup();
    render(
      <PermissionDock permissions={[makePerm({ id: "p1" })]} onReply={onReply} />
    );
    await user.click(screen.getByLabelText("Reject"));
    expect(onReply).toHaveBeenCalledWith("p1", "reject");
  });

  // ── Keyboard shortcuts ───────────────────────────────
  it("Enter key calls onReply with 'once'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".dock-card")!;
    fireEvent.keyDown(card, { key: "Enter" });
    expect(onReply).toHaveBeenCalledWith("pk", "once");
  });

  it("'a' key calls onReply with 'always'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".dock-card")!;
    fireEvent.keyDown(card, { key: "a" });
    expect(onReply).toHaveBeenCalledWith("pk", "always");
  });

  it("'A' key also calls onReply with 'always'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".dock-card")!;
    fireEvent.keyDown(card, { key: "A" });
    expect(onReply).toHaveBeenCalledWith("pk", "always");
  });

  it("Escape key calls onReply with 'reject'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".dock-card")!;
    fireEvent.keyDown(card, { key: "Escape" });
    expect(onReply).toHaveBeenCalledWith("pk", "reject");
  });

  it("'r' key calls onReply with 'reject'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".dock-card")!;
    fireEvent.keyDown(card, { key: "r" });
    expect(onReply).toHaveBeenCalledWith("pk", "reject");
  });

  it("does not let card shortcuts override button keyboard activation", () => {
    render(<PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />);
    fireEvent.keyDown(screen.getByLabelText("Always allow"), { key: "Enter" });
    expect(onReply).not.toHaveBeenCalled();
  });

  it("renders three action buttons", () => {
    render(
      <PermissionDock permissions={[makePerm()]} onReply={onReply} />
    );
    expect(screen.getByText("Allow Once")).toBeTruthy();
    expect(screen.getByText("Always Allow")).toBeTruthy();
    expect(screen.getByText("Reject")).toBeTruthy();
  });

  it("renders keyboard hint text", () => {
    render(
      <PermissionDock permissions={[makePerm()]} onReply={onReply} />
    );
    expect(screen.getByText("Enter = allow · A = always · Esc = reject")).toBeTruthy();
  });
});
