import type { QuestionItem } from "../../types";

// ── Upstream SSE → frontend type transformers ─────────────────────

/** Build a human-readable description for a permission request from upstream fields. */
export function formatPermissionDescription(props: Record<string, unknown>): string | undefined {
  // Use explicit description field if provided
  if (typeof props.description === "string" && props.description) {
    return props.description;
  }
  const permission = (props.permission ?? "") as string;
  const patterns = Array.isArray(props.patterns) ? (props.patterns as string[]) : [];

  const parts: string[] = [];
  if (permission) parts.push(permission);
  if (patterns.length > 0) parts.push(patterns.join(", "));
  return parts.length > 0 ? parts.join(": ") : undefined;
}

/** Derive a title for a question request from upstream fields.
 *  Upstream QuestionRequest has no `title`; we use the first question's header or fall back. */
export function deriveQuestionTitle(
  props: Record<string, unknown>,
  rawQuestions: unknown[],
): string {
  // Check if there's a title directly (future-proofing)
  if (typeof props.title === "string" && props.title) return props.title;
  // Use first question's header
  if (rawQuestions.length > 0) {
    const first = rawQuestions[0] as Record<string, unknown>;
    if (typeof first.header === "string" && first.header) return first.header;
  }
  return "Question";
}

/**
 * Transform an upstream QuestionInfo object to the frontend QuestionItem type.
 *
 * Upstream QuestionInfo: { question, header, options: [{label, description}], multiple, custom }
 * Frontend QuestionItem: { text, header, type, options: string[], optionDescriptions, multiple, custom }
 */
export function transformQuestionInfo(raw: unknown): QuestionItem {
  const info = raw as Record<string, unknown>;
  const question = (info.question ?? info.text ?? "") as string;
  const header = (info.header ?? "") as string;
  const multiple = Boolean(info.multiple);
  // `custom` defaults to true in upstream opencode (allows free-text alongside options)
  const custom = info.custom !== undefined ? Boolean(info.custom) : true;

  // Parse options array — upstream sends [{label, description}]
  const rawOptions = Array.isArray(info.options) ? info.options : [];
  const optionLabels: string[] = [];
  const optionDescs: string[] = [];
  for (const opt of rawOptions) {
    if (typeof opt === "string") {
      optionLabels.push(opt);
      optionDescs.push("");
    } else if (opt && typeof opt === "object") {
      const o = opt as Record<string, unknown>;
      optionLabels.push((o.label ?? "") as string);
      optionDescs.push((o.description ?? "") as string);
    }
  }

  // Derive the question type: respect upstream "confirm" type, otherwise infer from structure
  let type: QuestionItem["type"] = "text";
  if (info.type === "confirm") {
    type = "confirm";
  } else if (optionLabels.length > 0) {
    type = "select";
  }

  return {
    text: question,
    header: header || undefined,
    type,
    options: optionLabels.length > 0 ? optionLabels : undefined,
    optionDescriptions: optionDescs.some((d) => d.length > 0) ? optionDescs : undefined,
    multiple,
    custom,
  };
}
