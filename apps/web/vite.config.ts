import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The core runs in a module worker, which loads its WebAssembly through
// `new URL(…, import.meta.url)`, which Vite resolves itself.
export default defineConfig({
  plugins: [react()],
  // What tells this build from every other, for its service worker's cache (public/sw.js).
  define: { __BUILD__: JSON.stringify(Date.now().toString(36)) },
  worker: { format: "es" },
  build: {
    rollupOptions: {
      // The page imports only the core's enumerations' constants from its bindings (src/core),
      // which the worker runs whole: here they are taken as having no side effects, so the
      // page's bundle keeps the constants and leaves the rest to the worker's.
      treeshake: { moduleSideEffects: (id) => !id.includes("/src/core/") },
    },
  },
  server: { port: 5173, strictPort: true },
  preview: { port: 4173, strictPort: true },
});
