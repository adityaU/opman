// ── Types ─────────────────────────────────────────────

export interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  mem: number;
  status: string;
  /** Disk read bytes (total since process start). */
  disk_read: number;
  /** Disk write bytes (total since process start). */
  disk_write: number;
}

export interface DiskInfo {
  name: string;
  mount: string;
  total: number;
  used: number;
  fs_type: string;
}

export interface NetworkInfo {
  name: string;
  rx_bytes: number;
  tx_bytes: number;
}

export interface SystemStats {
  mem_total: number;
  mem_used: number;
  swap_total: number;
  swap_used: number;
  cpu_usage: number[];
  cpu_avg: number;
  uptime_secs: number;
  hostname: string;
  load_avg: [number, number, number];
  processes: ProcessInfo[];
  process_count: number;
  disks: DiskInfo[];
  networks: NetworkInfo[];
}

// ── SSE stream ────────────────────────────────────────

/**
 * Connect to the system stats SSE stream.
 * Returns an EventSource that emits "system_stats" events with SystemStats JSON payloads.
 * Caller is responsible for closing the EventSource when done.
 */
export function connectSystemStatsStream(
  onStats: (stats: SystemStats) => void,
  onError?: (err: Event) => void,
): EventSource {
  const es = new EventSource("/api/system/stats/stream");

  es.addEventListener("system_stats", (e: MessageEvent) => {
    try {
      const stats: SystemStats = JSON.parse(e.data);
      onStats(stats);
    } catch {
      // ignore parse errors
    }
  });

  if (onError) {
    es.onerror = onError;
  }

  return es;
}
