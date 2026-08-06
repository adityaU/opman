/**
 * Dev-only harness: serve the UI from source while proxying every API and
 * WebSocket call to a already-running opman instance. Lets frontend work be
 * inspected without restarting the binary that embeds web-ui/dist.
 *
 *   npx vite --config vite.preview.config.ts --port 5199
 *   OPMAN_ORIGIN=http://127.0.0.1:45511 overrides the target.
 */
import { defineConfig, mergeConfig } from "vite";
import base from "./vite.config";

const target = process.env.OPMAN_ORIGIN || "http://127.0.0.1:45511";

export default mergeConfig(base, defineConfig({
  server: {
    proxy: {
      "/api": { target, changeOrigin: false, ws: true },
      "/internal": { target, changeOrigin: false, ws: true },
    },
  },
}));
