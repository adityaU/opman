import { apiPost } from "../api/client";

/** Start a runner selected in the engine picker. Rejects with the server's own message. */
export async function startRunner(runner: string): Promise<void> {
  await apiPost("/runner/select", { runner });
}
