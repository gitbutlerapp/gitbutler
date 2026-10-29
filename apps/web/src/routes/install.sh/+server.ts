import { POSTHOG_API_KEY } from "$lib/analytics/posthogKey";
// Import the install script as a raw string using Vite's ?raw suffix
import installScript from "$scripts/install.sh?raw";
import { isIP } from "node:net";
import type { RequestEvent } from "./$types";

const RESPONSE_HEADERS = {
	"Content-Type": "text/plain; charset=utf-8",
	// No caching - users should always get the latest version
	// This is critical for security fixes and bug patches
	"Cache-Control": "no-cache, no-store, must-revalidate",
	Pragma: "no-cache",
	Expires: "0",
	// Security headers - defense in depth
	"Content-Security-Policy": "default-src 'none'",
	"X-Content-Type-Options": "nosniff",
};

export async function GET(event: RequestEvent) {
	await captureFetch(event);
	return new Response(installScript, { headers: RESPONSE_HEADERS });
}

// Counts script fetches server-side so the curl URL needs no query params.
// The await is required — Vercel kills promises still pending when the
// response returns — so the timeout caps how long a slow PostHog can delay
// the installer. Failures are logged but never break serving the script.
async function captureFetch({ request, url }: RequestEvent) {
	if (url.hostname !== "gitbutler.com") return;
	try {
		const ip = clientIp(request);
		const response = await fetch("https://eu.i.posthog.com/capture/", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({
				api_key: POSTHOG_API_KEY,
				event: "install_script_fetched",
				distinct_id: crypto.randomUUID(),
				properties: {
					$process_person_profile: false,
					...(ip ? { $ip: ip } : {}),
					$raw_user_agent: request.headers.get("user-agent") ?? "",
				},
			}),
			signal: AbortSignal.timeout(500),
		});
		if (!response.ok) console.error(`install.sh capture rejected: ${response.status}`);
		// Drain the connection so undici can return it to the pool.
		await response.body?.cancel();
	} catch (error) {
		console.error("install.sh capture failed:", error);
	}
}

// The last x-forwarded-for entry is the peer Vercel itself observed, so a
// client-supplied header can't spoof it; x-vercel-forwarded-for is the
// unspoofable client address when present. Undefined omits $ip entirely
// (sending $ip: null would disable PostHog's GeoIP for the event).
function clientIp(request: Request): string | undefined {
	const header =
		request.headers.get("x-vercel-forwarded-for") ?? request.headers.get("x-forwarded-for");
	const candidate = header?.split(",").at(-1)?.trim() ?? "";
	return isIP(candidate) ? candidate : undefined;
}
