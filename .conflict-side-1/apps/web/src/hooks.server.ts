import { fillMeta } from '$lib/meta/opengraph';

export async function handle({ event, resolve }) {
	const currentUrl = event.url.href;
	return resolve(event, {
		transformPageChunk: async ({ html }) => await fillMeta(html, currentUrl)
	});
}
