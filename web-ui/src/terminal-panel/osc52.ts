/**
 * OSC 52: a program asks the terminal to put text on the clipboard.
 *
 * tmux, Neovim and over-SSH tools use it so "copy" inside a terminal program reaches the
 * real clipboard. Only writes are honoured. A read request (`?`) would hand the clipboard's
 * contents to whatever program is running, so it is refused.
 */

const MAX_BYTES = 1_000_000;

/** The text an OSC 52 payload (`c;BASE64`) asks to copy, or `null` to ignore it. */
export function osc52Text(payload: string): string | null {
  const split = payload.indexOf(";");
  if (split < 0) return null;
  const data = payload.slice(split + 1);
  if (!data || data === "?" || data.length > MAX_BYTES * 1.4) return null;
  try {
    const binary = atob(data);
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return null;
  }
}
