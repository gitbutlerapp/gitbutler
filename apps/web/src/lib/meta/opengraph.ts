import { env } from '$env/dynamic/public';

export function fillMeta(html: string, url: string) {
	let metaTags = `
        <!-- Meta Tags -->
        <!-- Open Graph -->
        <meta property="og:title" content="GitButler" />
        <meta property="og:description" content="GitButler software development platform" />
        <meta property="og:image" content="%image%" />
        <meta property="og:url" content="${url}" />
        <meta property="og:site_name" content="GitButler" />

        <!-- Twitter Card Meta Tags -->
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content="GitButler" />
        <meta name="twitter:description" content="GitButler software development platform" />
        <meta name="twitter:image" content="%image%" />
        <meta name="twitter:site" content="@gitbutler" />
        <!-- / Meta Tags -->
`;

	const regex = /\/([^/]+)\/([^/]+)\/reviews\/([^/]+)/;
	const match = url.match(regex);
	if (match) {
		const [_, user, project, reviewId] = match;
		metaTags = metaTags.replaceAll(
			'%image%',
			`${env.PUBLIC_APP_HOST}og/review/${user}/${project}/${reviewId}`
		);
	} else {
		metaTags = metaTags.replaceAll('%image%', `${env.PUBLIC_APP_HOST}og/default`);
	}

	return html.replace('%metatags%', metaTags);
}
