import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/__tests__/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    css: false,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks: {
          "assistant-center": [
            "./src/AssistantCenterModal.tsx",
            "./src/recommendations.ts",
            "./src/resumeBriefing.ts",
            "./src/dailySummary.ts",
            "./src/handoffs.ts",
            "./src/inbox.ts",
          ],
          "assistant-modals": [
            "./src/InboxModal.tsx",
            "./src/MissionsModal.tsx",
            "./src/MemoryModal.tsx",
            "./src/AutonomyModal.tsx",
            "./src/RoutinesModal.tsx",
            "./src/DelegationBoardModal.tsx",
            "./src/WorkspaceManagerModal.tsx",
            "./src/NotificationPrefsModal.tsx",
          ],
          analytics: [
            "./src/SessionGraph.tsx",
            "./src/SessionDashboard.tsx",
            "./src/ActivityFeed.tsx",
          ],
          editor: [
            "./src/CodeEditorPanel.tsx",
            "@uiw/react-codemirror",
            "@codemirror/view",
            "@codemirror/state",
            "@codemirror/language",
            "@codemirror/language-data",
            "@codemirror/lint",
          ],
          terminal: [
            "@xterm/xterm",
            "@xterm/addon-fit",
            "@xterm/addon-search",
            "@xterm/addon-web-links",
          ],
        },
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:9090",
    },
  },
});
