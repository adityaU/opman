// Service worker for React UI — served at /ui/ scope.
// Satisfies Chrome PWA installability requirement.
// Intercepts icon/manifest requests to serve theme-aware versions.

// ── Theme state ────────────────────────────────────────────────────
let themeColors = null; // { primary: "#...", background: "#..." }

function buildThemeSvg(primary, bg) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="7" fill="${bg}"/>
  <path d="M7 22 L14 16 L7 10" stroke="${primary}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
  <line x1="16" y1="22" x2="25" y2="22" stroke="${primary}" stroke-width="2.5" stroke-linecap="round"/>
</svg>`;
}

async function svgToPng(svgText, size) {
  if (typeof OffscreenCanvas !== "undefined") {
    const blob = new Blob([svgText], { type: "image/svg+xml" });
    const bmp = await createImageBitmap(blob, { resizeWidth: size, resizeHeight: size });
    const canvas = new OffscreenCanvas(size, size);
    const ctx = canvas.getContext("2d");
    ctx.drawImage(bmp, 0, 0, size, size);
    bmp.close();
    return await canvas.convertToBlob({ type: "image/png" });
  }
  return null;
}

// ── Lifecycle ──────────────────────────────────────────────────────

self.addEventListener("install", function () {
  self.skipWaiting();
});

self.addEventListener("activate", function (event) {
  event.waitUntil(self.clients.claim());
});

// ── Message handler (theme updates + notifications) ────────────────

self.addEventListener("message", function (event) {
  if (!event.data || !event.data.type) return;

  if (event.data.type === "THEME_COLORS") {
    themeColors = event.data.colors;
    return;
  }

  if (event.data.type === "SHOW_NOTIFICATION") {
    var payload = event.data.payload || {};
    var title = payload.title || "opman";
    var options = {
      body: payload.body || "",
      icon: "/ui/favicon.svg",
      tag: payload.tag || "opman-" + Date.now(),
      silent: false,
      data: {
        sessionId: payload.sessionId || null,
        kind: payload.kind || null,
        url: payload.url || "/ui/",
      },
    };
    self.registration.showNotification(title, options);
    return;
  }
});

// ── Notification click handler ─────────────────────────────────────

self.addEventListener("notificationclick", function (event) {
  event.notification.close();

  var data = event.notification.data || {};
  var targetUrl = data.url || "/ui/";

  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then(function (clientList) {
      for (var i = 0; i < clientList.length; i++) {
        var client = clientList[i];
        if (client.url.indexOf(self.location.origin) === 0 && "focus" in client) {
          client.postMessage({
            type: "NOTIFICATION_CLICK",
            sessionId: data.sessionId,
            kind: data.kind,
          });
          return client.focus();
        }
      }
      if (self.clients.openWindow) {
        return self.clients.openWindow(targetUrl);
      }
    })
  );
});

// ── Fetch handler ──────────────────────────────────────────────────

self.addEventListener("fetch", function (event) {
  const url = new URL(event.request.url);

  if (url.origin !== self.location.origin) return;

  const path = url.pathname;

  // Themed favicon.svg
  if (path === "/ui/favicon.svg" && themeColors) {
    event.respondWith(
      (async () => {
        try {
          const svg = buildThemeSvg(themeColors.primary, themeColors.background);
          return new Response(svg, {
            headers: {
              "Content-Type": "image/svg+xml",
              "Cache-Control": "no-cache",
            },
          });
        } catch {
          return fetch(event.request);
        }
      })()
    );
    return;
  }

  // Themed PNG icons
  if ((path === "/ui/icon-192.png" || path === "/ui/icon-512.png") && themeColors) {
    const size = path === "/ui/icon-192.png" ? 192 : 512;
    event.respondWith(
      (async () => {
        try {
          const svg = buildThemeSvg(themeColors.primary, themeColors.background);
          const pngBlob = await svgToPng(svg, size);
          if (pngBlob) {
            return new Response(pngBlob, {
              headers: {
                "Content-Type": "image/png",
                "Cache-Control": "no-cache",
              },
            });
          }
        } catch {
          // fall through
        }
        return fetch(event.request);
      })()
    );
    return;
  }

  // Themed manifest.json
  if (path === "/ui/manifest.json" && themeColors) {
    event.respondWith(
      (async () => {
        try {
          const res = await fetch(event.request);
          const manifest = await res.json();
          manifest.theme_color = themeColors.background;
          manifest.background_color = themeColors.background;
          return new Response(JSON.stringify(manifest), {
            headers: {
              "Content-Type": "application/manifest+json",
              "Cache-Control": "no-cache",
            },
          });
        } catch {
          return fetch(event.request);
        }
      })()
    );
    return;
  }

  // Navigation requests — network-first with offline fallback
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
