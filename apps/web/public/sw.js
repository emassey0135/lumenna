// Lumenna's service worker: the app opens with no network, as a local-first app should, its
// store being in this browser already (OPFS). The page itself is fetched afresh while online,
// so an update arrives at the next load; every other file of this origin, the scripts and the
// core's WebAssembly, is kept as it is first fetched, their names carrying their contents'
// hash. Syncing (other origins: the relays) is never touched.

const CACHE = "lumenna";

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(["/", "/manifest.webmanifest"])));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) return;
  if (request.mode === "navigate") {
    // The page: the network's, kept for when there is none.
    event.respondWith(
      fetch(request)
        .then((response) => {
          const kept = response.clone();
          void caches.open(CACHE).then((cache) => cache.put("/", kept));
          return response;
        })
        .catch(() => caches.match("/")),
    );
    return;
  }
  event.respondWith(
    caches.match(request).then(
      (kept) =>
        kept ??
        fetch(request).then((response) => {
          if (response.ok) {
            const copy = response.clone();
            void caches.open(CACHE).then((cache) => cache.put(request, copy));
          }
          return response;
        }),
    ),
  );
});

// A reminder's notification, clicked: back to the app, which shows the reminder due.
self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((windows) => {
      const open = windows[0];
      if (open) {
        open.postMessage({ reminder: event.notification.tag });
        return open.focus();
      }
      return self.clients.openWindow("/");
    }),
  );
});
