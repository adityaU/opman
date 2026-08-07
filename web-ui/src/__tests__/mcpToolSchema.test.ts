import { describe, expect, it } from "vitest";
import {
  constraintsOf,
  fieldsOf,
  signatureOf,
  typeOf,
  type JsonSchema,
} from "../settings-page/mcp/schema";

/**
 * A tool schema is the one thing on the MCP settings page opman did not author, so these
 * cover the shapes real servers actually send — not just the ones the reader was written
 * against.
 */

describe("typeOf", () => {
  it("names the primitive", () => {
    expect(typeOf({ type: "string" })).toBe("string");
  });

  it("renders an enum as its members, because for an argument they are the type", () => {
    expect(typeOf({ type: "string", enum: ["immediate", "queued"] })).toBe(
      '"immediate" | "queued"',
    );
  });

  it("summarises an enum too long to spell out", () => {
    const many = { enum: ["a", "b", "c", "d", "e", "f"] };
    expect(typeOf(many)).toBe('"a" | "b" | "c" | "d" | +2 more');
  });

  it("suffixes an array with its item type", () => {
    expect(typeOf({ type: "array", items: { type: "string" } })).toBe("string[]");
  });

  it("parenthesises a union inside an array so the brackets cannot be misread", () => {
    expect(typeOf({ type: "array", items: { enum: ["a", "b"] } })).toBe('("a" | "b")[]');
  });

  it("joins a nullable union", () => {
    expect(typeOf({ type: ["string", "null"] })).toBe("string | null");
  });

  it("reads oneOf and anyOf as unions", () => {
    expect(typeOf({ anyOf: [{ type: "string" }, { type: "number" }] })).toBe("string | number");
  });

  it("falls back to object when only properties are given", () => {
    expect(typeOf({ properties: { a: { type: "string" } } })).toBe("object");
  });

  /* A blank cell would be indistinguishable from a cell we failed to fill. */
  it("says any rather than nothing for a schema it cannot read", () => {
    expect(typeOf({})).toBe("any");
    expect(typeOf(undefined)).toBe("any");
  });
});

describe("constraintsOf", () => {
  it("collapses a two-sided numeric bound into a range", () => {
    expect(constraintsOf({ minimum: 1, maximum: 10 })).toEqual(["1–10"]);
  });

  it("uses a comparison for a one-sided bound", () => {
    expect(constraintsOf({ minimum: 0 })).toEqual(["≥ 0"]);
    expect(constraintsOf({ maximum: 64 })).toEqual(["≤ 64"]);
  });

  it("labels length and item counts with their unit", () => {
    expect(constraintsOf({ minLength: 1, maxLength: 64 })).toEqual(["1–64 chars"]);
    expect(constraintsOf({ maxItems: 32 })).toEqual(["max 32 items"]);
  });

  it("carries format and pattern through", () => {
    expect(constraintsOf({ format: "uri", pattern: "^tsk_" })).toEqual([
      "uri",
      "matches ^tsk_",
    ]);
  });

  it("says nothing when the schema states no bounds", () => {
    expect(constraintsOf({ type: "string" })).toEqual([]);
  });
});

describe("fieldsOf", () => {
  const tool: JsonSchema = {
    type: "object",
    properties: {
      delivery: { type: "string", enum: ["immediate", "queued"], default: "immediate" },
      message: { type: "string", description: "The message text." },
      model: { type: "string" },
    },
    required: ["message", "model"],
  };

  /* A caller reads the required arguments and stops, and JSON object order is whatever
     the server happened to serialise. */
  it("puts the required parameters first", () => {
    expect(fieldsOf(tool).map((field) => field.name)).toEqual(["message", "model", "delivery"]);
  });

  it("marks which parameters may be omitted", () => {
    const byName = Object.fromEntries(fieldsOf(tool).map((field) => [field.name, field]));
    expect(byName.message.required).toBe(true);
    expect(byName.delivery.required).toBe(false);
  });

  it("renders a stated default as a literal", () => {
    const delivery = fieldsOf(tool).find((field) => field.name === "delivery");
    expect(delivery?.fallback).toBe('"immediate"');
  });

  it("descends into a nested object so a structured argument is not just object", () => {
    const nested = fieldsOf({
      properties: { where: { type: "object", properties: { x: { type: "number" } } } },
    });
    expect(nested[0].fields.map((field) => field.name)).toEqual(["x"]);
  });

  it("descends into an array's items, which is where list schemas put their keys", () => {
    const rows = fieldsOf({
      properties: {
        fields: { type: "array", items: { type: "object", properties: { selector: {} } } },
      },
    });
    expect(rows[0].fields.map((field) => field.name)).toEqual(["selector"]);
  });

  it("returns nothing for a schema with no properties at all", () => {
    expect(fieldsOf({ type: "object" })).toEqual([]);
    expect(fieldsOf(undefined)).toEqual([]);
  });
});

describe("signatureOf", () => {
  it("marks the optional arguments with a question mark", () => {
    const schema: JsonSchema = {
      properties: { task_id: { type: "string" }, body: { type: "string" } },
      required: ["task_id", "body"],
    };
    expect(signatureOf("kanban_add_note", schema)).toBe("kanban_add_note(task_id, body)");
  });

  it("keeps required arguments ahead of optional ones", () => {
    const schema: JsonSchema = {
      properties: { runner: {}, model: {}, effort: {} },
      required: ["model", "effort"],
    };
    expect(signatureOf("agent_start", schema)).toBe("agent_start(model, effort, runner?)");
  });

  it("shows empty parentheses for a tool that takes nothing", () => {
    expect(signatureOf("skill_list", { type: "object" })).toBe("skill_list()");
    expect(signatureOf("skill_list", undefined)).toBe("skill_list()");
  });
});
