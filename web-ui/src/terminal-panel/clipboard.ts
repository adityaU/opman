/**
 * Reading and writing the system clipboard from a terminal.
 *
 * `navigator.clipboard` needs a secure page and, for reading, the user's say-so. Writing
 * falls back to a hidden textarea and `execCommand("copy")`, which works from a click or key
 * press on any page. Reading has no such fallback, so it reports `null` and the caller can
 * tell the user to press the paste key instead.
 */

export async function copyText(text: string): Promise<boolean> {
  if (!text) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return copyWithSelection(text);
  }
}

function copyWithSelection(text: string): boolean {
  const holder = document.createElement("textarea");
  holder.value = text;
  holder.setAttribute("readonly", "");
  holder.style.cssText = "position:fixed;top:0;left:0;opacity:0;pointer-events:none";
  const previous = document.activeElement;
  document.body.appendChild(holder);
  holder.select();
  let copied = false;
  try {
    copied = document.execCommand("copy");
  } catch {
    copied = false;
  }
  holder.remove();
  if (previous instanceof HTMLElement) previous.focus();
  return copied;
}

/** The clipboard's text, or `null` when the browser will not say. */
export async function readText(): Promise<string | null> {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
}
