import { esc, sf, chartColor, fmtNum } from "./types";
import type { ChartDataset, ChartValue } from "./types";

const W = 400, H = 200, PAD = 30;

function renderLegend(datasets: ChartDataset[]): string {
  if (datasets.length <= 1) return "";
  let h = '<div class="a2ui-chart-legend">';
  datasets.forEach((ds, i) => {
    const c = ds.color || chartColor(i);
    h += `<span class="a2ui-chart-legend-item"><span class="a2ui-chart-legend-dot" style="background:${c}"></span>${esc(ds.label ?? `Series ${i + 1}`)}</span>`;
  });
  return h + "</div>";
}

function safeData(ds: ChartDataset): number[] {
  return Array.isArray(ds.data) ? ds.data : [];
}

function computeRange(datasets: ChartDataset[]): [number, number] {
  let min = Infinity, max = -Infinity;
  for (const ds of datasets) {
    for (const v of safeData(ds)) {
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }
  if (min === max) { min -= 1; max += 1; }
  return [min, max];
}

function yGridLines(min: number, max: number, count = 4): number[] {
  const step = (max - min) / count;
  const lines: number[] = [];
  for (let i = 0; i <= count; i++) lines.push(min + step * i);
  return lines;
}

function scaleY(v: number, min: number, max: number): number {
  return H - PAD - ((v - min) / (max - min)) * (H - 2 * PAD);
}

function renderLineArea(data: Record<string, unknown>, area: boolean): string {
  const labels = (data.labels as string[]) ?? [];
  const datasets = (data.datasets as ChartDataset[]) ?? [];
  if (!datasets.length || !labels.length) return "";
  const [min, max] = computeRange(datasets);
  const n = labels.length;
  const xStep = n > 1 ? (W - 2 * PAD) / (n - 1) : 0;

  let svg = `<svg class="a2ui-chart-svg" viewBox="0 0 ${W} ${H}">`;
  // grid
  for (const yv of yGridLines(min, max)) {
    const y = scaleY(yv, min, max);
    svg += `<line class="a2ui-chart-grid" x1="${PAD}" y1="${y}" x2="${W - PAD}" y2="${y}"/>`;
    svg += `<text class="a2ui-chart-y-label" x="${PAD - 4}" y="${y + 3}">${fmtNum(yv)}</text>`;
  }
  // x labels
  labels.forEach((l, i) => {
    const x = PAD + i * xStep;
    svg += `<text class="a2ui-chart-label" x="${x}" y="${H - 4}">${esc(l)}</text>`;
  });
  // datasets
  datasets.forEach((ds, di) => {
    const c = ds.color || chartColor(di);
    const safe = safeData(ds);
    const pts = safe.map((v, i) => `${PAD + i * xStep},${scaleY(v, min, max)}`);
    if (area) {
      const first = `${PAD},${H - PAD}`;
      const last = `${PAD + (n - 1) * xStep},${H - PAD}`;
      svg += `<polygon points="${first} ${pts.join(" ")} ${last}" fill="${c}" opacity="0.15"/>`;
    }
    svg += `<polyline points="${pts.join(" ")}" fill="none" stroke="${c}" stroke-width="2"/>`;
    // dots
    safe.forEach((v, i) => {
      svg += `<circle cx="${PAD + i * xStep}" cy="${scaleY(v, min, max)}" r="3" fill="${c}"/>`;
    });
  });
  svg += "</svg>";
  return svg + renderLegend(datasets);
}

function renderBar(data: Record<string, unknown>): string {
  const labels = (data.labels as string[]) ?? [];
  const datasets = (data.datasets as ChartDataset[]) ?? [];
  if (!datasets.length || !labels.length) return "";
  const [min, max] = computeRange(datasets);
  const n = labels.length;
  const groupW = (W - 2 * PAD) / n;
  const barW = Math.max(4, groupW / (datasets.length + 1));

  let svg = `<svg class="a2ui-chart-svg" viewBox="0 0 ${W} ${H}">`;
  for (const yv of yGridLines(min, max)) {
    const y = scaleY(yv, min, max);
    svg += `<line class="a2ui-chart-grid" x1="${PAD}" y1="${y}" x2="${W - PAD}" y2="${y}"/>`;
    svg += `<text class="a2ui-chart-y-label" x="${PAD - 4}" y="${y + 3}">${fmtNum(yv)}</text>`;
  }
  labels.forEach((l, i) => {
    const cx = PAD + groupW * i + groupW / 2;
    svg += `<text class="a2ui-chart-label" x="${cx}" y="${H - 4}">${esc(l)}</text>`;
  });
  datasets.forEach((ds, di) => {
    const c = ds.color || chartColor(di);
    safeData(ds).forEach((v, i) => {
      const x = PAD + groupW * i + barW * di + (groupW - barW * datasets.length) / 2;
      const y = scaleY(v, min, max);
      const bH = (H - PAD) - y;
      svg += `<rect x="${x}" y="${y}" width="${barW}" height="${bH}" fill="${c}" rx="2"/>`;
    });
  });
  svg += "</svg>";
  return svg + renderLegend(datasets);
}

function renderPieDonut(data: Record<string, unknown>, donut: boolean): string {
  const values = (data.values as ChartValue[]) ?? [];
  if (!values.length) return "";
  const total = values.reduce((s, v) => s + Math.abs(v.value), 0);
  if (total === 0) return "";
  const cx = 100, cy = 100, r = 80, inner = donut ? 50 : 0;
  let svg = `<svg class="a2ui-chart-svg" viewBox="0 0 200 200" style="max-height:200px">`;
  let angle = -Math.PI / 2;
  values.forEach((v, i) => {
    const slice = (Math.abs(v.value) / total) * Math.PI * 2;
    const x1 = cx + r * Math.cos(angle);
    const y1 = cy + r * Math.sin(angle);
    const x2 = cx + r * Math.cos(angle + slice);
    const y2 = cy + r * Math.sin(angle + slice);
    const large = slice > Math.PI ? 1 : 0;
    const c = v.color || chartColor(i);
    if (donut) {
      const ix1 = cx + inner * Math.cos(angle);
      const iy1 = cy + inner * Math.sin(angle);
      const ix2 = cx + inner * Math.cos(angle + slice);
      const iy2 = cy + inner * Math.sin(angle + slice);
      svg += `<path d="M${x1} ${y1} A${r} ${r} 0 ${large} 1 ${x2} ${y2} L${ix2} ${iy2} A${inner} ${inner} 0 ${large} 0 ${ix1} ${iy1}Z" fill="${c}"/>`;
    } else {
      svg += `<path d="M${cx} ${cy} L${x1} ${y1} A${r} ${r} 0 ${large} 1 ${x2} ${y2}Z" fill="${c}"/>`;
    }
    angle += slice;
  });
  svg += "</svg>";
  // legend
  let legend = '<div class="a2ui-chart-legend">';
  values.forEach((v, i) => {
    const c = v.color || chartColor(i);
    legend += `<span class="a2ui-chart-legend-item"><span class="a2ui-chart-legend-dot" style="background:${c}"></span>${esc(v.label)} (${fmtNum(v.value)})</span>`;
  });
  legend += "</div>";
  return svg + legend;
}

export function renderChart(data: Record<string, unknown>): string {
  const chartType = sf(data, "chart_type") || "bar";
  const title = sf(data, "title");
  let h = '<div class="a2ui-chart">';
  if (title) h += `<div class="a2ui-chart-title">${esc(title)}</div>`;
  switch (chartType) {
    case "line": h += renderLineArea(data, false); break;
    case "area": h += renderLineArea(data, true); break;
    case "bar": h += renderBar(data); break;
    case "pie": h += renderPieDonut(data, false); break;
    case "donut": h += renderPieDonut(data, true); break;
    default: h += `<div class="a2ui-unknown">Unsupported chart: ${esc(chartType)}</div>`;
  }
  h += "</div>";
  return h;
}
