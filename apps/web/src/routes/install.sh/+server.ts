// Import the install script as a raw string using Vite's ?raw suffix
import installScript from "$scripts/install.sh?raw";

export function GET() {
	return new Response(installScript, {
		headers: {
			"Content-Type": "text/plain; charset=utf-8",
			// No caching - users should always get the latest version
			// This is critical for security fixes and bug patches
			"Cache-Control": "no-cache, no-store, must-revalidate",
			Pragma: "no-cache",
			Expires: "0",
			// Security headers - defense in depth
			"Content-Security-Policy": "default-src 'none'",
			"X-Content-Type-Options": "nosniff",
		},
	});
}
