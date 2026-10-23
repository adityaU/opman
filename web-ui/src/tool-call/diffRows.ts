export type DiffRow = { type: "ctx" | "add" | "del"; text: string };

/** Bound diff work so large rewrites cannot allocate a quadratic table. */
export function lineDiff(oldText: string, newText: string): DiffRow[] {
  const a = oldText.length ? oldText.split("\n") : [];
  const b = newText.length ? newText.split("\n") : [];
  const rows: DiffRow[] = [];
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) {
    rows.push({ type: "ctx", text: a[prefix++] });
  }
  let oldEnd = a.length;
  let newEnd = b.length;
  while (oldEnd > prefix && newEnd > prefix && a[oldEnd - 1] === b[newEnd - 1]) {
    oldEnd--;
    newEnd--;
  }
  const m = oldEnd - prefix;
  const n = newEnd - prefix;
  if ((m + 1) * (n + 1) > 250_000) {
    for (let i = prefix; i < oldEnd; i++) rows.push({ type: "del", text: a[i] });
    for (let j = prefix; j < newEnd; j++) rows.push({ type: "add", text: b[j] });
  } else {
    const width = n + 1;
    const dp = new Uint32Array((m + 1) * width);
    for (let i = m - 1; i >= 0; i--) {
      for (let j = n - 1; j >= 0; j--) {
        dp[i * width + j] = a[prefix + i] === b[prefix + j]
          ? dp[(i + 1) * width + j + 1] + 1
          : Math.max(dp[(i + 1) * width + j], dp[i * width + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < m && j < n) {
      if (a[prefix + i] === b[prefix + j]) {
        rows.push({ type: "ctx", text: a[prefix + i++] });
        j++;
      } else if (dp[(i + 1) * width + j] >= dp[i * width + j + 1]) {
        rows.push({ type: "del", text: a[prefix + i++] });
      } else {
        rows.push({ type: "add", text: b[prefix + j++] });
      }
    }
    while (i < m) rows.push({ type: "del", text: a[prefix + i++] });
    while (j < n) rows.push({ type: "add", text: b[prefix + j++] });
  }
  for (let i = oldEnd; i < a.length; i++) rows.push({ type: "ctx", text: a[i] });
  return rows;
}
