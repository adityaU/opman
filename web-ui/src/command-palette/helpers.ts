import { PaletteItem } from "./types";

/** Filter palette items by search query */
export function filterItems(items: PaletteItem[], query: string): PaletteItem[] {
  if (!query) return items;
  const lq = query.toLowerCase();
  return items.filter(
    (i) =>
      i.label.toLowerCase().includes(lq) ||
      i.description?.toLowerCase().includes(lq)
  );
}

/** Group filtered items by category (globally, preserving first-seen order) */
export function groupItems(filtered: PaletteItem[]): Array<{ category: string; items: PaletteItem[] }> {
  const map = new Map<string, PaletteItem[]>();
  for (const item of filtered) {
    const existing = map.get(item.category);
    if (existing) {
      existing.push(item);
    } else {
      map.set(item.category, [item]);
    }
  }
  const sections: Array<{ category: string; items: PaletteItem[] }> = [];
  for (const [category, items] of map) {
    sections.push({ category, items });
  }
  return sections;
}
