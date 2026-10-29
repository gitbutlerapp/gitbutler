import { SITEMAP_PATHS } from "$lib/meta/opengraph";

export function GET({ url }) {
	const urls = SITEMAP_PATHS.map((path) => `\t<url><loc>${url.origin}${path}</loc></url>`).join(
		"\n",
	);
	const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>
`;

	return new Response(body, {
		headers: {
			"Content-Type": "application/xml",
			"Cache-Control": "max-age=3600",
		},
	});
}
