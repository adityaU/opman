import type { CommandDef } from "../types";

/** Session lifecycle, navigation and the session-overview surfaces. */
export const SESSION_COMMANDS: readonly CommandDef[] = [
  { id: "session.new", title: "New Session", category: "Sessions", label: "new" },
  { id: "session.newInProject", title: "New Session in Project…", category: "Sessions", label: "new in project" },
  { id: "session.switch", title: "Switch Session…", category: "Sessions", label: "switch" },
  { id: "session.next", title: "Next Session", category: "Sessions" },
  { id: "session.previous", title: "Previous Session", category: "Sessions" },
  { id: "session.filterSidebar", title: "Filter Sessions", category: "Sessions", label: "filter" },
  { id: "session.rename", title: "Rename Session", category: "Sessions", when: "sessionActive", label: "rename" },
  { id: "session.togglePin", title: "Pin or Unpin Session", category: "Sessions", when: "sessionActive", label: "pin" },
  { id: "session.close", title: "Close Session", category: "Sessions", when: "sessionActive", label: "close" },
  { id: "session.delete", title: "Delete Session", category: "Sessions", when: "sessionActive", label: "delete" },
  { id: "session.fork", title: "Fork Session", category: "Sessions", when: "sessionActive", label: "fork" },
  { id: "session.share", title: "Share Session", category: "Sessions", when: "sessionActive", label: "share" },
  { id: "session.watcher", title: "Session Watcher", category: "Sessions", label: "watcher" },
];
