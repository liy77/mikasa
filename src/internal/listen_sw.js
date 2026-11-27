/* Mocidad listen / site service worker — emitted by mikasa.
   Cache-first for heavy immutable assets (?v=BUILD_ID); network-first for
   HTML / build-id / gate. Placeholders: __ASSET_VER__ / __BUILD_ID__. */

var CACHE = "mocidad-__BUILD_ID__";
var VER = "__BUILD_ID__";
var BUILT_AT = "__BUILT_AT__";

/* Warm light shell only — never precache index.html (a sticky HTML shell kept
   clients on a stale BUILD_ID / old wasm and Docs OOBd forever). Wasm/js/png
   land in CACHE via fetchAndCache / page fetch wrapper. */
var PRECACHE = [
  "./build-id.json",
  "./boot.js?v=" + VER,
  "./sw.js",
  "./manifest.json?v=" + VER,
  "./logo.png?v=" + VER
];

function putBoth(cache, req, res) {
  if (!res || !res.ok) return Promise.resolve();
  var copy = res.clone();
  var jobs = [cache.put(req, copy)];
  try {
    var u = typeof req === "string" ? req : req.url;
    if (u) jobs.push(cache.put(u, res.clone()));
  } catch (e) { /* ignore */ }
  return Promise.all(jobs).catch(function () { return null; });
}

function fetchAndCache(req) {
  return fetch(req).then(function (res) {
    if (res && res.ok) {
      var copy = res.clone();
      caches.open(CACHE).then(function (c) { putBoth(c, req, copy); });
    }
    return res;
  });
}

self.addEventListener("install", function (event) {
  event.waitUntil(
    caches.open(CACHE).then(function (cache) {
      return Promise.all(
        PRECACHE.map(function (url) {
          return fetch(url)
            .then(function (res) {
              if (res && res.ok) return cache.put(url, res);
            })
            .catch(function () { return null; });
        })
      );
    }).then(function () {
      return self.skipWaiting();
    })
  );
});

self.addEventListener("activate", function (event) {
  event.waitUntil(
    caches.keys().then(function (keys) {
      return Promise.all(
        keys.map(function (key) {
          if (key.indexOf("mocidad-") === 0 && key !== CACHE) {
            return caches.delete(key);
          }
          return null;
        })
      );
    }).then(function () {
      return self.clients.claim();
    })
  );
});

function isHeavy(url) {
  return /\.(wasm|js|ttf|otf|woff2?|png|jpe?g|webp|gif)(\?|$)/i.test(url.pathname) ||
    /mui_dev\.wasm|mui-dev\.js|boot\.js/i.test(url.pathname);
}

function isShell(url) {
  return url.pathname === "/" ||
    /\/index\.html$|\/build-gate\.js$|\/build-id\.json$|\/app\.mui$|\/main\.mika$|\/manifest\.json$|\/sw\.js$/i.test(url.pathname) ||
    /\.(mui|mika|crm)$/i.test(url.pathname);
}

function matchCached(req, url) {
  return caches.match(req).then(function (hit) {
    if (hit) return hit;
    return caches.match(url.href).then(function (hit2) {
      if (hit2) return hit2;
      /* Same path, any ?v= — survives soft reloads / stale query strings. */
      return caches.match(req, { ignoreSearch: true }).then(function (hit3) {
        if (hit3) return hit3;
        try {
          return caches.match(url.pathname + url.search, { ignoreSearch: true });
        } catch (e) {
          return null;
        }
      });
    });
  });
}

self.addEventListener("fetch", function (event) {
  var req = event.request;
  if (req.method !== "GET") return;
  var url;
  try {
    url = new URL(req.url);
  } catch (e) {
    return;
  }
  if (url.origin !== self.location.origin) return;

  if (isShell(url)) {
    event.respondWith(
      fetch(req)
        .then(function (res) {
          if (res && res.ok) {
            var copy = res.clone();
            caches.open(CACHE).then(function (c) { putBoth(c, req, copy); });
          }
          return res;
        })
        .catch(function () {
          return matchCached(req, url).then(function (hit) {
            return hit || caches.match("./index.html");
          });
        })
    );
    return;
  }

  if (!isHeavy(url)) return;

  /* Images: ignoreSearch so warmFetch(?v=BUILD) answers emscripten_fetch
     without a query. Wasm/js: network-first — a stale/truncated Cache Storage
     entry was aborting boot with "both async and sync fetching of wasm failed"
     on mobile (instantiateStreaming + sync XHR both fail). */
  var isImage = /\.(png|jpe?g|webp|gif|svg)(\?|$)/i.test(url.pathname);
  var isWasmOrJs = /\.(wasm|js)(\?|$)/i.test(url.pathname)
    || /mui_dev\.wasm|mui-dev\.js|boot\.js/i.test(url.pathname);
  if (isWasmOrJs) {
    event.respondWith(
      fetchAndCache(req).catch(function () {
        return matchCached(req, url).then(function (hit) {
          return hit || Response.error();
        });
      })
    );
    return;
  }
  event.respondWith(
    (isImage ? matchCached(req, url) : caches.match(req).then(function (hit) {
      return hit || caches.match(url.href);
    })).then(function (hit) {
      if (hit) {
        var verOk = url.search.indexOf("v=" + VER) >= 0;
        if (!verOk) {
          return fetchAndCache(req);
        }
        return hit;
      }
      return fetchAndCache(req);
    })
  );
});
