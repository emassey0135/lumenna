import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import react from "@vitejs/plugin-react";

/**
 * What Lumenna is, from the workspace's one source (`[workspace.metadata.lumenna]` in the root
 * Cargo.toml), which every platform's build reads: a bump there is the web's too. A table of
 * plain `key = "value"` lines, read without a TOML library.
 */
function metadata(): Record<string, string> {
  const cargo = readFileSync(new URL("../../Cargo.toml", import.meta.url), "utf8");
  const table = cargo.split(/^\[workspace\.metadata\.lumenna\]\s*$/m)[1]?.split(/^\[/m)[0] ?? "";
  const found: Record<string, string> = {};
  for (const [, key, value] of table.matchAll(/^(\w+)\s*=\s*"([^"]*)"\s*$/gm)) found[key] = value;
  return found;
}

/**
 * The web app's manifest: `manifest.json` beside this file (its icons and launch shortcuts),
 * its name and description the workspace's, served by the dev server and written into a build
 * at `/manifest.webmanifest`. The source is named otherwise because Vite bundles a file the
 * page links that it finds at that path, under a hashed name: the service worker and an
 * installed app need the address to stay.
 */
function manifest(): Plugin {
  const text = () => {
    const { name, description } = metadata();
    const own = JSON.parse(readFileSync(new URL("./manifest.json", import.meta.url), "utf8"));
    return JSON.stringify({ name, short_name: name, description, ...own }, null, 2) + "\n";
  };
  return {
    name: "lumenna-manifest",
    configureServer(server) {
      server.middlewares.use("/manifest.webmanifest", (_request, response) => {
        response.setHeader("Content-Type", "application/manifest+json");
        response.end(text());
      });
    },
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "manifest.webmanifest", source: text() });
    },
  };
}

// The core runs in a module worker, which loads its WebAssembly through
// `new URL(…, import.meta.url)`, which Vite resolves itself.
export default defineConfig({
  plugins: [react(), manifest()],
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
