/** One key in flight at a time, so Neovim's replies stay in step with sends. */
export class InputQueue {
  private inFlight = false;
  private readonly pending: string[] = [];

  push(keys: string): void {
    this.pending.push(keys);
  }

  acknowledge(): void {
    this.inFlight = false;
  }

  clear(): void {
    this.inFlight = false;
    this.pending.length = 0;
  }

  /** Send the next key when `ready`. Returns false only when the send failed. */
  pump(ready: boolean, send: (keys: string) => boolean): boolean {
    if (this.inFlight || !ready) return true;
    const keys = this.pending.shift();
    if (keys === undefined) return true;
    this.inFlight = true;
    if (send(keys)) return true;
    this.inFlight = false;
    return false;
  }
}
