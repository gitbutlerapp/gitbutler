/**
 * Where signing in may send the reader back to, from a `callback` the login page was given:
 * an address on this site, or on a local development server. Anything else is undefined, so a
 * link cannot send someone elsewhere once they have signed in. GitButler's API checks it again.
 */
export function safeCallback(value: string | null, site: string): string | undefined {
	if (!value) return undefined;
	try {
		const url = new URL(value);
		const local = url.hostname === "localhost" || url.hostname === "127.0.0.1";
		return url.origin === new URL(site).origin || local ? url.toString() : undefined;
	} catch {
		return undefined;
	}
}
