import { describe, expect, it } from "vitest";
import { CORE_RENDERERS } from "./blocks";

describe("A2UI alert block", () => {
  it("renders the canonical message field", () => {
    const html = CORE_RENDERERS.alert({ level: "warning", message: "Deployment is pending." });
    expect(html).toContain("Deployment is pending.");
  });

  it("keeps body aliases visible instead of rendering a blank alert", () => {
    const html = CORE_RENDERERS.alert({ level: "warning", body: "Approval is required." });
    expect(html).toContain("Approval is required.");
  });
});
