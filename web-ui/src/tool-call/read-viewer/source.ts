import { rawFileUrl, fileDownloadUrl } from "../../api";

/**
 * Where a viewer gets its bytes.
 *
 * `inline` is a data URL the tool result already carried, so nothing is fetched.
 * `server` is a path on this opman server, read through its raw-file route.
 */
export type FileSource =
  | { readonly kind: "inline"; readonly url: string }
  | { readonly kind: "server"; readonly path: string; readonly url: string };

const DATA_IMAGE = /^\s*(data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=\r\n]+)\s*$/i;
const DATA_MEDIA = /^\s*(data:(?:image\/[a-z0-9.+-]+|video\/[a-z0-9.+-]+|audio\/[a-z0-9.+-]+|application\/pdf|model\/stl);base64,[A-Za-z0-9+/=\r\n]+)\s*$/i;

/** An image data URL that a tool put straight in its output, if there is one. */
export function inlineImageUrl(output: string): string | null {
  if (output.length < 24 || output.length > 40_000_000) return null;
  const match = DATA_IMAGE.exec(output);
  return match ? match[1].replace(/[\r\n]+/g, "") : null;
}

export function sourceFor(path: string, output: string): FileSource | null {
  const inline = output.length <= 40_000_000 ? DATA_MEDIA.exec(output)?.[1]?.replace(/[\r\n]+/g, "") : null;
  if (inline) return { kind: "inline", url: inline };
  if (!path) return null;
  return { kind: "server", path, url: rawFileUrl(path) };
}

/** A link that saves the file, or null when the bytes only exist inline. */
export function downloadHref(source: FileSource): string | null {
  return source.kind === "server" ? fileDownloadUrl(source.path) : null;
}
