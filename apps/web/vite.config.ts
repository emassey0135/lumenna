import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The core runs in a module worker; its WebAssembly is loaded by the bindings wasm-bindgen
// writes, through `new URL(…, import.meta.url)`, which Vite resolves itself.
export default defineConfig({
  plugins: [react()],
  worker: { format: "es" },
  server: { port: 5173, strictPort: true },
  preview: { port: 4173, strictPort: true },
});
