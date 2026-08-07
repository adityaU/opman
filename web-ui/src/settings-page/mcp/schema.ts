/**
 * A tool's JSON Schema, read for display.
 *
 * A tool definition is the one thing on this page opman did not author — it is whatever the
 * server sent — so nothing here assumes a shape. Every reader returns something printable
 * for input it does not recognise, because a schema that renders as a blank cell is worse
 * than one that renders as `any`: the user cannot tell the difference between "no
 * constraint" and "we failed to read it".
 *
 * The raw source stays available alongside this, so being approximate here is safe.
 */

/** How many enum members to spell out before summarising the rest. */
const ENUM_LIMIT = 4;

export interface JsonSchema {
  readonly type?: string | string[];
  readonly properties?: Record<string, JsonSchema>;
  readonly required?: string[];
  readonly items?: JsonSchema;
  readonly enum?: unknown[];
  readonly const?: unknown;
  readonly oneOf?: JsonSchema[];
  readonly anyOf?: JsonSchema[];
  readonly allOf?: JsonSchema[];
  readonly description?: string;
  readonly default?: unknown;
  readonly format?: string;
  readonly pattern?: string;
  readonly minimum?: number;
  readonly maximum?: number;
  readonly minLength?: number;
  readonly maxLength?: number;
  readonly minItems?: number;
  readonly maxItems?: number;
  readonly additionalProperties?: boolean | JsonSchema;
}

/** One parameter, flattened into the row the table renders. */
export interface Field {
  readonly name: string;
  /** A printable type expression — `string`, `string[]`, `"a" | "b"`. */
  readonly type: string;
  readonly required: boolean;
  readonly description?: string;
  /** Rendered default, when the schema states one. */
  readonly fallback?: string;
  /** Range, length, pattern and format, as short phrases. */
  readonly constraints: readonly string[];
  /** A nested object's own fields, so a structured argument is not just `object`. */
  readonly fields: readonly Field[];
}

function isSchema(value: unknown): value is JsonSchema {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function literal(value: unknown): string {
  return typeof value === "string" ? `"${value}"` : JSON.stringify(value) ?? String(value);
}

/**
 * A printable type for one schema node.
 *
 * An enum is rendered as its members rather than as `string`: for a tool argument the
 * members *are* the type, and that is the single most useful thing the table can say.
 */
export function typeOf(schema: JsonSchema | undefined): string {
  if (!schema) return "any";
  if (schema.const !== undefined) return literal(schema.const);
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    const shown = schema.enum.slice(0, ENUM_LIMIT).map(literal).join(" | ");
    const rest = schema.enum.length - ENUM_LIMIT;
    return rest > 0 ? `${shown} | +${rest} more` : shown;
  }
  const union = schema.oneOf ?? schema.anyOf;
  if (union && union.length > 0) return union.map(typeOf).join(" | ");
  if (Array.isArray(schema.type)) return schema.type.join(" | ");
  if (schema.type === "array") {
    const item = typeOf(schema.items);
    // `("a" | "b")[]` — parenthesised so the brackets cannot read as part of the union.
    return item.includes(" | ") ? `(${item})[]` : `${item}[]`;
  }
  if (schema.type) return schema.type;
  if (schema.properties) return "object";
  if (schema.allOf?.length) return schema.allOf.map(typeOf).join(" & ");
  return "any";
}

/** The stated bounds, as phrases short enough to sit in a table cell. */
export function constraintsOf(schema: JsonSchema): string[] {
  const out: string[] = [];
  const range = (min: number | undefined, max: number | undefined, unit: string) => {
    if (min !== undefined && max !== undefined) out.push(`${min}–${max} ${unit}`);
    else if (min !== undefined) out.push(`min ${min} ${unit}`);
    else if (max !== undefined) out.push(`max ${max} ${unit}`);
  };
  if (schema.minimum !== undefined && schema.maximum !== undefined) {
    out.push(`${schema.minimum}–${schema.maximum}`);
  } else if (schema.minimum !== undefined) {
    out.push(`≥ ${schema.minimum}`);
  } else if (schema.maximum !== undefined) {
    out.push(`≤ ${schema.maximum}`);
  }
  range(schema.minLength, schema.maxLength, "chars");
  range(schema.minItems, schema.maxItems, "items");
  if (schema.format) out.push(schema.format);
  if (schema.pattern) out.push(`matches ${schema.pattern}`);
  return out;
}

/**
 * A schema's properties as table rows, required ones first.
 *
 * The ordering is the point: a caller reads the required arguments and stops, and JSON
 * object order is whatever the server happened to serialise.
 */
export function fieldsOf(schema: JsonSchema | undefined, depth = 0): Field[] {
  if (!schema?.properties || depth > 2) return [];
  const required = new Set(schema.required ?? []);
  return Object.entries(schema.properties)
    .filter(([, child]) => isSchema(child))
    .map(([name, child]) => ({
      name,
      type: typeOf(child),
      required: required.has(name),
      description: child.description,
      fallback: child.default === undefined ? undefined : literal(child.default),
      constraints: constraintsOf(child),
      fields: fieldsOf(child.type === "array" ? child.items : child, depth + 1),
    }))
    .sort((a, b) => Number(b.required) - Number(a.required));
}

/**
 * A one-line call signature: `kanban_add_note(task_id, body, priority?)`.
 *
 * This is what makes the collapsed list worth reading — it answers "what does this take"
 * without opening anything, and marks which of those the caller may omit.
 */
export function signatureOf(name: string, schema: JsonSchema | undefined): string {
  const fields = fieldsOf(schema);
  if (fields.length === 0) return `${name}()`;
  const args = fields.map((field) => (field.required ? field.name : `${field.name}?`));
  return `${name}(${args.join(", ")})`;
}
