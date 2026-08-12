export const CONTROL_MSG_TYPES = [
  "ready",
  "error",
  "exited",
  "superseded",
  "too_slow",
] as const;

export const CLIENT_MSG_TYPES = ["input", "input_mouse", "resize", "paste"] as const;

// Named aliases make the protocol inventory straightforward for contract tests.
export const CONTROL_VARIANT_NAMES = CONTROL_MSG_TYPES;
export const CLIENT_VARIANT_NAMES = CLIENT_MSG_TYPES;
export const WIRE_VARIANT_NAMES = [...CONTROL_MSG_TYPES, ...CLIENT_MSG_TYPES] as const;

export type ControlMsg =
  | { readonly type: "ready" }
  | { readonly type: "error"; readonly message: string }
  | { readonly type: "exited"; readonly code: number | null }
  | { readonly type: "superseded" }
  | { readonly type: "too_slow" };

export type ClientMsg =
  | { readonly type: "input"; readonly keys: string }
  | {
      readonly type: "input_mouse";
      readonly button: string;
      readonly action: string;
      readonly modifier: string;
      readonly grid: number;
      readonly row: number;
      readonly col: number;
    }
  | { readonly type: "resize"; readonly rows: number; readonly cols: number }
  | { readonly type: "paste"; readonly data: string };

export type WireMessage = ControlMsg | ClientMsg;
