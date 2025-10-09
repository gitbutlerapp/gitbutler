import { env } from '$env/dynamic/public';

function replacePropertyContent(metaTags: string, property: string, newContent: string) {
	const regexOg = new RegExp(`property="og:${property}" content="([^"]+)"`);
	metaTags = metaTags.replace(regexOg, `property="og:${property}" content="${newContent}"`);
	const regexTwitter = new RegExp(`name="twitter:${property}" content="([^"]+)"`);
	metaTags = metaTags.replace(regexTwitter, `name="twitter:${property}" content="${newContent}"`);
	return metaTags;
}

export async function fillMeta(html: string, url: string) {
	let metaTags = `
        <!-- Meta Tags -->
        <!-- Open Graph -->
        <meta property="og:title" content="GitButler" />
        <meta property="og:description" content="GitButler software development platform" />
        <meta property="og:image" content="%image%" />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
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

	const regex_patch = /\/([^/]+)\/([^/]+)\/reviews\/([^/]+)\/commit\/([^/]+)/;
	let match = url.match(regex_patch);
	if (match) {
		const [_, user, project, reviewId, changeId] = match;
		metaTags = metaTags.replaceAll(
			'%image%',
			`${env.PUBLIC_APP_HOST}og/review/${user}/${project}/${reviewId}/${changeId}`
		);

		try {
			const response = await fetch(
				env.PUBLIC_APP_HOST +
					`api/patch_stack/${user}/${project}/branch/${reviewId}/commit/${changeId}`
			);
			const data = await response.json();
			metaTags = replacePropertyContent(metaTags, 'title', `Review ${data.title}`);
			if (data.description) {
				metaTags = replacePropertyContent(metaTags, 'description', data.description);
			} else {
				metaTags = replacePropertyContent(
					metaTags,
					'description',
					`Review code for ${user}/${project}`
				);
			}
			return html.replace('%metatags%', metaTags);
		} catch (error: unknown) {
			console.error('Fetch error:', error);
			return html.replace('%metatags%', metaTags);
		}
	}

	const regex_review = /\/([^/]+)\/([^/]+)\/reviews\/([^/]+)/;
	match = url.match(regex_review);
	if (match) {
		const [_, user, project, reviewId] = match;
		metaTags = metaTags.replaceAll(
			'%image%',
			`${env.PUBLIC_APP_HOST}og/review/${user}/${project}/${reviewId}`
		);

		// hit the API for this patch and get the project name
		try {
			const response = await fetch(
				env.PUBLIC_APP_HOST + `api/patch_stack/${user}/${project}/branch/${reviewId}`
			);
			const data = await response.json();
			metaTags = replacePropertyContent(metaTags, 'title', `Review ${data.title}`);
			if (data.description) {
				metaTags = replacePropertyContent(metaTags, 'description', data.description);
			} else {
				metaTags = replacePropertyContent(
					metaTags,
					'description',
					`Review code for ${user}/${project}`
				);
			}
			return html.replace('%metatags%', metaTags);
		} catch (error: unknown) {
			console.error('Fetch error:', error);
			return html.replace('%metatags%', metaTags);
		}
	}

	// Default fallback for non-review pages
	metaTags = metaTags.replaceAll('%image%', `${env.PUBLIC_APP_HOST}og-image.png`);
	return html.replace('%metatags%', metaTags);
}
