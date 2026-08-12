import { apiPost } from "./client";

export type NvimReadOp =
  | "nvim_buffers"
  | "nvim_info"
  | "nvim_diagnostics"
  | "nvim_definition"
  | "nvim_references"
  | "nvim_hover"
  | "nvim_symbols"
  | "nvim_code_actions"
  | "nvim_grep"
  | "nvim_diff"
  | "nvim_signature";

export type NvimEditOp =
  | "nvim_open"
  | "nvim_read"
  | "nvim_input"
  | "nvim_write"
  | "nvim_edit_and_save"
  | "nvim_undo"
  | "nvim_rename"
  | "nvim_format";

/** Browser-safe operations. Execute-capability operations are intentionally absent. */
export type NvimOp = NvimReadOp | NvimEditOp;

export interface NvimRequest {
  session_id: string;
  op: NvimOp;
  file_path?: string;
  line?: number;
  end_line?: number;
  input?: string;
  new_text?: string;
  edits?: Array<{ file_path: string; start_line: number; end_line: number; new_text: string }>;
  query?: string;
  glob?: string;
  count?: number;
  buf_only?: boolean;
  workspace?: boolean;
}

export interface NvimResponse {
  ok: boolean;
  output?: string;
  error?: string;
}

export function nvim(request: NvimRequest): Promise<NvimResponse> {
  return apiPost<NvimResponse>("/nvim", request);
}
