import { handleAttributionCookies } from '$lib/cookies/attribution';
import { fillMeta } from '$lib/meta/opengraph';

export async function handle({ event, resolve }) {
	const currentUrl = event.url.href;

	// Handle attribution cookies (referrer and first page)
	handleAttributionCookies(event);

	return resolve(event, {
		transformPageChunk: async ({ html }) => await fillMeta(html, currentUrl)
	});
}
