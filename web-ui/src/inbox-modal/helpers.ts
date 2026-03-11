import type { InboxItemSource } from "../api/intelligence";

export function formatSource(source: InboxItemSource): string {
  switch (source) {
    case "permission":
      return "Permission";
    case "question":
      return "Question";
    case "mission":
      return "Paused mission";
    case "watcher":
      return "Watcher";
    case "completion":
      return "Completion";
  }
}

export function formatTime(time: number): string {
  return new Date(time).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}
