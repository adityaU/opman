/**
 * One id generator for the whole app.
 *
 * `crypto.randomUUID` needs a secure context, which opman is not always served
 * from (a plain-http LAN address is a normal way to reach it), so the fallback
 * is not decorative.
 */
export function uuid(): string {
  return (
    crypto.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
  );
}
