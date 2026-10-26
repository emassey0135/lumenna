import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

// Installed, or opened with no network, the app comes from its service worker (public/sw.js).
// Not under Vite's dev server, whose modules change under it. Each build's worker has an address
// of its own, so a new build installs it again and it clears the caches of those before.
if (import.meta.env.PROD && "serviceWorker" in navigator) {
  void navigator.serviceWorker.register(`/sw.js?build=${encodeURIComponent(__BUILD__)}`).catch(() => undefined);
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
