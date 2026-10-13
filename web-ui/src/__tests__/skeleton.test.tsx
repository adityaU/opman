import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { Skeleton, SkeletonRows } from "../skeleton/Skeleton";

function stubReducedMotion(reduce: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: reduce && query.includes("prefers-reduced-motion"),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

afterEach(() => vi.unstubAllGlobals());

describe("Skeleton", () => {
  it("sweeps by default", () => {
    stubReducedMotion(false);
    const { container } = render(<Skeleton width={40} />);
    const block = container.querySelector(".skel")!;
    expect(block).toHaveAttribute("data-motion", "sweep");
    expect(block).toHaveAttribute("aria-hidden", "true");
    expect(block).toHaveStyle({ width: "40px", height: "10px" });
  });

  it("holds still under prefers-reduced-motion", () => {
    stubReducedMotion(true);
    const { container } = render(<SkeletonRows label="Loading models" count={3} />);
    const blocks = container.querySelectorAll(".skel");
    expect(blocks.length).toBe(6);
    blocks.forEach((block) => expect(block).toHaveAttribute("data-motion", "static"));
  });

  it("announces what is loading once, as a busy status", () => {
    stubReducedMotion(false);
    render(<SkeletonRows label="Loading agents" count={2} lead="dot" />);
    const status = screen.getByRole("status", { name: /Loading agents/ });
    expect(status).toHaveAttribute("aria-busy", "true");
    expect(status.querySelectorAll(".skel--circle")).toHaveLength(2);
  });
});
