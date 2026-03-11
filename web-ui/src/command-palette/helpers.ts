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

/** Group filtered items by category */
export function groupItems(filtered: PaletteItem[]): Array<{ category: string; items: PaletteItem[] }> {
  const sections: Array<{ category: string; items: PaletteItem[] }> = [];
  for (const item of filtered) {
    const current = sections[sections.length - 1];
    if (current && current.category === item.category) {
      current.items.push(item);
    } else {
      sections.push({ category: item.category, items: [item] });
    }
  }
  return sections;
}
