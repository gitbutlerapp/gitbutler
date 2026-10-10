// Keeps the panel's own page at hand for when `but panel` isn't running, so the browser shows the
// panel saying so, rather than its own "can't be reached" page. Only the page, its script and its
// icon are kept; everything under /api/ is always live, or fails, and is never served from here.
//
// The page and script are fetched from the server first and kept only as a fallback, so a new
// version of `but` shows on the next load, not the one after.

const CACHE = "but-panel-shell";
const SHELL = ["/", "/app.js", "/icon.svg"];

self.addEventListener("install", (event) => {
	event.waitUntil(
		caches
			.open(CACHE)
			.then((cache) => cache.addAll(SHELL))
			.then(() => self.skipWaiting()),
	);
});

self.addEventListener("activate", (event) => {
	event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
	const url = new URL(event.request.url);
	if (event.request.method !== "GET" || url.origin !== self.location.origin) return;
	// A page for any project is the same page: the project comes from the address.
	const shell = event.request.mode === "navigate" ? "/" : url.pathname;
	if (!SHELL.includes(shell)) return;

	event.respondWith(
		fetch(event.request)
			.then((response) => {
				if (response.ok) {
					const copy = response.clone();
					caches.open(CACHE).then((cache) => cache.put(shell, copy));
				}
				return response;
			})
			.catch(() => caches.match(shell)),
	);
});
