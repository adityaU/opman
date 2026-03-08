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
    expect(screen.getByText("bash")).toBeTruthy();
    expect(screen.getByText("write")).toBeTruthy();
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

  it("renders args when provided", () => {
    render(
      <PermissionDock
        permissions={[makePerm({ args: { cmd: "ls -la" } })]}
        onReply={onReply}
      />
    );
    expect(screen.getByText(/"cmd": "ls -la"/)).toBeTruthy();
  });

  it("does not render args when empty object", () => {
    const { container } = render(
      <PermissionDock permissions={[makePerm({ args: {} })]} onReply={onReply} />
    );
    expect(container.querySelector(".permission-args")).toBeNull();
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
    const card = document.querySelector(".permission-card")!;
    fireEvent.keyDown(card, { key: "Enter" });
    expect(onReply).toHaveBeenCalledWith("pk", "once");
  });

  it("'a' key calls onReply with 'always'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".permission-card")!;
    fireEvent.keyDown(card, { key: "a" });
    expect(onReply).toHaveBeenCalledWith("pk", "always");
  });

  it("'A' key also calls onReply with 'always'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".permission-card")!;
    fireEvent.keyDown(card, { key: "A" });
    expect(onReply).toHaveBeenCalledWith("pk", "always");
  });

  it("Escape key calls onReply with 'reject'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".permission-card")!;
    fireEvent.keyDown(card, { key: "Escape" });
    expect(onReply).toHaveBeenCalledWith("pk", "reject");
  });

  it("'r' key calls onReply with 'reject'", () => {
    render(
      <PermissionDock permissions={[makePerm({ id: "pk" })]} onReply={onReply} />
    );
    const card = document.querySelector(".permission-card")!;
    fireEvent.keyDown(card, { key: "r" });
    expect(onReply).toHaveBeenCalledWith("pk", "reject");
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
    expect(screen.getByText("Enter=allow, A=always, Esc=reject")).toBeTruthy();
  });
});
