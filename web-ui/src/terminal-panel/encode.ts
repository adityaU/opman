/** Encode terminal input for `POST /pty/{id}/write`, which takes base64 bytes. */
export function encodeForPty(data: string): string {
  const bytes = new TextEncoder().encode(data);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
