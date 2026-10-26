// Lumenna's service worker: the app opens with no network, as a local-first app should, its
// store being in this browser already (OPFS). The page itself is fetched afresh while online,
// so an update arrives at the next load, and so are the manifest and the icons, whose names
// carry no hash; every other file of this origin, the scripts and the core's WebAssembly, is
// kept as it is first fetched, their names carrying their contents' hash. Syncing (other
// origins: the relays) is never touched.
//
// Each build registers this worker under its own address (`?build=`, main.tsx), so a new build
// installs it afresh and keeps its files in a cache of its own; the caches of the builds before
// it are deleted as it takes over, since what they hold shares the origin's storage with the
// store, and a browser short of space may evict the store with them.

const BUILD = new URL(self.location.href).searchParams.get("build") ?? "unbuilt";
const PREFIX = "lumenna";
const CACHE = `${PREFIX}-${BUILD}`;

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(["/", "/manifest.webmanifest"])));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((name) => name.startsWith(PREFIX) && name !== CACHE).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  );
});

/** The network's answer to `request`, kept under `key` when it is a whole one; what was kept
 * when there is no network. */
function fresh(request, key) {
  return fetch(request)
    .then((response) => {
      if (response.ok) {
        const kept = response.clone();
        void caches.open(CACHE).then((cache) => cache.put(key, kept));
      }
      return response;
    })
    .catch(() => caches.match(key).then((kept) => kept ?? Response.error()));
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin) return;
  // The page: the network's, kept for when there is none; an error page is never kept as it.
  if (request.mode === "navigate") {
    event.respondWith(fresh(request, "/"));
    return;
  }
  // Named without a hash, so a new one would never be fetched once kept.
  if (url.pathname === "/manifest.webmanifest" || url.pathname.startsWith("/icons/")) {
    event.respondWith(fresh(request, request));
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
