import React, { useState } from "react";
import { Check, Copy } from "lucide-react";
import type { McpTool } from "../../api/mcp";
import { fieldsOf, type Field } from "./schema";

/**
 * One tool, in full.
 *
 * The parameter table is the deliverable: an agent calling this tool needs the argument
 * names, which of them it may omit, and what values are legal — and those are three
 * different columns, not three sentences to parse out of a description.
 *
 * The schema source sits underneath rather than instead. The table is a reading of the
 * schema and the reading is lossy by design; anyone who needs the exact document has it
 * one disclosure away, and can copy it.
 */

export interface ToolDefinitionProps {
  readonly tool: McpTool;
}

export function ToolDefinition({ tool }: ToolDefinitionProps) {
  const fields = fieldsOf(tool.inputSchema);

  return (
    <div className="mcpt-def">
      {tool.description && <p className="mcpt-def-prose">{tool.description}</p>}

      {fields.length === 0 ? (
        <p className="mcpt-def-none">Takes no arguments.</p>
      ) : (
        <table className="mcpt-params">
          <thead>
            <tr>
              <th scope="col">Parameter</th>
              <th scope="col">Type</th>
              <th scope="col">Notes</th>
            </tr>
          </thead>
          <tbody>
            {fields.map((field) => (
              <ParamRow key={field.name} field={field} />
            ))}
          </tbody>
        </table>
      )}

      <SchemaSource tool={tool} />
    </div>
  );
}

/**
 * One parameter.
 *
 * `Required` is a word, not a colour or a dot — the distinction between an argument you
 * must pass and one you may is the single most consequential thing in the table, and it
 * has to survive being read in greyscale or by a screen reader.
 */
function ParamRow({ field }: { readonly field: Field }) {
  return (
    <tr>
      <th scope="row">
        <code className="mcpt-param-name">{field.name}</code>
        <span className={field.required ? "mcpt-need is-required" : "mcpt-need"}>
          {field.required ? "Required" : "Optional"}
        </span>
      </th>
      <td>
        <code className="mcpt-param-type">{field.type}</code>
      </td>
      <td>
        {field.description && <span className="mcpt-param-prose">{field.description}</span>}
        <span className="mcpt-param-facts">
          {field.fallback !== undefined && (
            <span>
              defaults to <code>{field.fallback}</code>
            </span>
          )}
          {field.constraints.map((constraint) => (
            <span key={constraint}>{constraint}</span>
          ))}
          {field.fields.length > 0 && (
            <span>
              keys: {field.fields.map((child) => child.name).join(", ")}
            </span>
          )}
        </span>
      </td>
    </tr>
  );
}

/** The schema exactly as the server sent it, foldable and copyable. */
function SchemaSource({ tool }: { readonly tool: McpTool }) {
  const [copied, setCopied] = useState(false);
  const source = JSON.stringify(
    { inputSchema: tool.inputSchema, outputSchema: tool.outputSchema, annotations: tool.annotations },
    (_key, value) => (value === undefined ? undefined : value),
    2,
  );

  const copy = () => {
    navigator.clipboard.writeText(source).then(
      () => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1600);
      },
      () => setCopied(false),
    );
  };

  return (
    <details className="mcpt-source">
      <summary>Schema source</summary>
      <div className="mcpt-source-body">
        <button
          type="button"
          className="stg-icon-btn mcpt-copy"
          onClick={copy}
          aria-label={`Copy the schema for ${tool.name}`}
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}
        </button>
        <pre>
          <code>{source}</code>
        </pre>
      </div>
    </details>
  );
}
