import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  base: "/ui/",
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
        manualChunks(id) {
          // React core + JSX runtime must be in their own chunk to avoid
          // circular-dependency issues at module initialization time.
          if (
            id.includes("node_modules/react/") ||
            id.includes("node_modules/react-dom/") ||
            id.includes("node_modules/react-is/") ||
            id.includes("node_modules/scheduler/")
          ) {
            return "vendor-react";
          }

          // Lucide icons (shared across many chunks)
          if (id.includes("node_modules/lucide-react/")) {
            return "vendor-icons";
          }

          // CodeMirror / editor
          if (
            id.includes("node_modules/@codemirror/") ||
            id.includes("node_modules/@uiw/react-codemirror") ||
            id.includes("node_modules/@lezer/") ||
            id.includes("node_modules/crelt/") ||
            id.includes("node_modules/style-mod/") ||
            id.includes("node_modules/w3c-keyname/")
          ) {
            return "editor";
          }
          if (id.includes("/src/code-editor/")) return "editor";

          // Terminal (xterm)
          if (id.includes("node_modules/@xterm/")) return "terminal";

          // App-level lazy chunks
          if (id.includes("/src/AssistantCenterModal")) return "assistant-center";
          if (id.includes("/src/api/intelligence")) return "assistant-center";

          if (
            id.includes("/src/InboxModal") ||
            id.includes("/src/MissionsModal") ||
            id.includes("/src/MemoryModal") ||
            id.includes("/src/AutonomyModal") ||
            id.includes("/src/RoutinesModal") ||
            id.includes("/src/DelegationBoardModal") ||
            id.includes("/src/WorkspaceManagerModal") ||
            id.includes("/src/NotificationPrefsModal")
          ) {
            return "assistant-modals";
          }

          if (
            id.includes("/src/SessionGraph") ||
            id.includes("/src/SessionDashboard") ||
            id.includes("/src/ActivityFeed")
          ) {
            return "analytics";
          }
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
