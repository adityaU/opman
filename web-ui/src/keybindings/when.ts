/**
 * `when` clause evaluation.
 *
 * A deliberately small language — identifiers, `==`, `!=`, `!`, `&&`, `||` and
 * parentheses — because a clause is read on every keystroke and a binding that
 * needs more than this is a binding whose surface should be publishing a
 * clearer context key instead.
 *
 *   sessionActive
 *   focus==editor && !editorDirty
 *   permissionPending || questionPending
 */

export type WhenContext = Readonly<Record<string, string | boolean | undefined>>;

type Token = { kind: "id" | "op" | "paren"; text: string };

const OPERATORS = ["&&", "||", "==", "!=", "!"];

export class WhenParseError extends Error {}

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < input.length) {
    const char = input[i];
    if (char === " " || char === "\t") {
      i += 1;
      continue;
    }
    if (char === "(" || char === ")") {
      tokens.push({ kind: "paren", text: char });
      i += 1;
      continue;
    }
    const operator = OPERATORS.find((op) => input.startsWith(op, i));
    if (operator) {
      tokens.push({ kind: "op", text: operator });
      i += operator.length;
      continue;
    }
    const match = /^[A-Za-z0-9_.\-/]+/.exec(input.slice(i));
    if (!match) throw new WhenParseError(`unexpected character "${char}" in "${input}"`);
    tokens.push({ kind: "id", text: match[0] });
    i += match[0].length;
  }

  return tokens;
}

/** Truthiness of a context value: a present, non-empty, non-false value. */
function truthy(value: string | boolean | undefined): boolean {
  if (value === undefined) return false;
  if (typeof value === "boolean") return value;
  return value.length > 0 && value !== "false";
}

class Parser {
  private index = 0;

  constructor(
    private readonly tokens: readonly Token[],
    private readonly ctx: WhenContext,
    private readonly source: string,
  ) {}

  private peek(): Token | undefined {
    return this.tokens[this.index];
  }

  private take(): Token {
    const token = this.tokens[this.index];
    if (!token) throw new WhenParseError(`unexpected end of "${this.source}"`);
    this.index += 1;
    return token;
  }

  /** `a || b` — lowest precedence. */
  parseOr(): boolean {
    let left = this.parseAnd();
    while (this.peek()?.text === "||") {
      this.take();
      // Both sides are evaluated: the parser must consume the right operand
      // whether or not the result is already decided.
      const right = this.parseAnd();
      left = left || right;
    }
    return left;
  }

  private parseAnd(): boolean {
    let left = this.parseComparison();
    while (this.peek()?.text === "&&") {
      this.take();
      const right = this.parseComparison();
      left = left && right;
    }
    return left;
  }

  private parseComparison(): boolean {
    const token = this.peek();
    if (token?.kind === "id" && this.tokens[this.index + 1]?.kind === "op") {
      const operator = this.tokens[this.index + 1].text;
      if (operator === "==" || operator === "!=") {
        this.take();
        this.take();
        const right = this.take();
        if (right.kind !== "id") {
          throw new WhenParseError(`expected a value after ${operator} in "${this.source}"`);
        }
        const equal = String(this.ctx[token.text] ?? "") === right.text;
        return operator === "==" ? equal : !equal;
      }
    }
    return this.parseUnary();
  }

  private parseUnary(): boolean {
    if (this.peek()?.text === "!") {
      this.take();
      return !this.parseUnary();
    }
    return this.parsePrimary();
  }

  private parsePrimary(): boolean {
    const token = this.take();
    if (token.text === "(") {
      const value = this.parseOr();
      const close = this.take();
      if (close.text !== ")") throw new WhenParseError(`expected ")" in "${this.source}"`);
      return value;
    }
    if (token.kind !== "id") {
      throw new WhenParseError(`unexpected "${token.text}" in "${this.source}"`);
    }
    if (token.text === "true") return true;
    if (token.text === "false") return false;
    return truthy(this.ctx[token.text]);
  }

  get done(): boolean {
    return this.index >= this.tokens.length;
  }
}

/**
 * Evaluate a clause. An absent clause is always true.
 *
 * A malformed clause evaluates to `false` rather than throwing: the clause may
 * have come from a hand-edited config file, and one typo should cost the user
 * that binding, not every keystroke in the app.
 */
export function evaluateWhen(clause: string | undefined, ctx: WhenContext): boolean {
  if (!clause) return true;
  try {
    const parser = new Parser(tokenize(clause), ctx, clause);
    const value = parser.parseOr();
    if (!parser.done) throw new WhenParseError(`trailing input in "${clause}"`);
    return value;
  } catch (error) {
    if (error instanceof WhenParseError) return false;
    throw error;
  }
}

/** Build a memoized evaluator for one context snapshot. */
export function whenEvaluator(ctx: WhenContext): (clause: string) => boolean {
  const cache = new Map<string, boolean>();
  return (clause: string) => {
    const cached = cache.get(clause);
    if (cached !== undefined) return cached;
    const value = evaluateWhen(clause, ctx);
    cache.set(clause, value);
    return value;
  };
}
