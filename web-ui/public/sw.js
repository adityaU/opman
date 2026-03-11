// Service worker — satisfies Chrome PWA installability requirement.
// Network-only: no caching. The sole purpose is to make browsers treat the
// app as an installable PWA so it runs in true standalone display mode.

self.addEventListener("install", function () {
  self.skipWaiting();
});

self.addEventListener("activate", function (event) {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", function (event) {
  // Only intercept same-origin navigation requests.
  // Everything else falls through to the browser default.
  if (event.request.mode === "navigate") {
    event.respondWith(
      fetch(event.request).catch(function () {
        return new Response(
          "<!DOCTYPE html><html><body><p>Offline</p></body></html>",
          { headers: { "Content-Type": "text/html" } }
        );
      })
    );
  }
});
