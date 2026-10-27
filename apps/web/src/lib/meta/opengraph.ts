import { env } from "$env/dynamic/public";

function replacePropertyContent(metaTags: string, property: string, newContent: string) {
	const regexOg = new RegExp(`property="og:${property}" content="([^"]+)"`);
	metaTags = metaTags.replace(regexOg, `property="og:${property}" content="${newContent}"`);
	const regexTwitter = new RegExp(`name="twitter:${property}" content="([^"]+)"`);
	metaTags = metaTags.replace(regexTwitter, `name="twitter:${property}" content="${newContent}"`);
	return metaTags;
}

interface PageMeta {
	title: string;
	description: string;
	/** Another page that shows the same content and should be indexed instead. */
	canonicalPath?: string;
}

const DEFAULT_META: PageMeta = {
	title: "GitButler",
	description:
		"GitButler is a Git client for modern, AI-assisted development: parallel and stacked branches, unlimited undo and agent integrations, on top of your existing Git repositories.",
};

// What search engines and link previews show for the marketing pages.
const PAGE_META: Record<string, PageMeta> = {
	"/": {
		title: "GitButler | Version control for your agents",
		description:
			"GitButler is a Git CLI and desktop client built for humans and AI coding agents. Stacked and parallel branches, absorb, unlimited undo and stacked pull requests, on any Git repository.",
	},
	"/client": {
		title: "GitButler | Desktop",
		description:
			"The GitButler desktop client: work on parallel and stacked branches at once, drag changes between commits, undo anything and open stacked pull requests, all on top of Git.",
	},
	"/downloads": {
		title: "Download GitButler",
		description:
			"Download the GitButler desktop client for macOS, Windows and Linux, with release notes for every version.",
	},
	"/releases": {
		title: "GitButler releases",
		description: "Release notes and downloads for every GitButler release.",
		canonicalPath: "/downloads",
	},
	"/nightly": {
		title: "GitButler nightly builds",
		description: "Download the latest GitButler nightly builds for macOS, Windows and Linux.",
	},
};

/** The marketing pages that are indexed under their own address. */
export const SITEMAP_PATHS = Object.entries(PAGE_META)
	.filter(([, meta]) => !meta.canonicalPath)
	.map(([path]) => path);

/**
 * The title `fillMeta` writes for a path. Pages put the same title in
 * `<svelte:head>`, because Svelte sets `document.title` from it in the browser.
 */
export function pageTitle(pathname: string) {
	return (PAGE_META[pathname] ?? DEFAULT_META).title;
}

const HOME_STRUCTURED_DATA = JSON.stringify({
	"@context": "https://schema.org",
	"@type": "SoftwareApplication",
	name: "GitButler",
	applicationCategory: "DeveloperApplication",
	operatingSystem: "macOS, Windows, Linux",
	description: PAGE_META["/"].description,
	url: "https://gitbutler.com/",
	offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
});

export async function fillMeta(html: string, url: string) {
	const { pathname, origin } = new URL(url);
	const pageMeta = PAGE_META[pathname];
	const { title, description } = pageMeta ?? DEFAULT_META;
	let metaTags = `
        <title>${title}</title>
        <meta name="description" content="${description}" />
        ${pageMeta ? `<link rel="canonical" href="${origin}${pageMeta.canonicalPath ?? pathname}" />` : ""}
        ${pathname === "/" ? `<script type="application/ld+json">${HOME_STRUCTURED_DATA}</script>` : ""}

        <!-- Meta Tags -->
        <!-- Open Graph -->
        <meta property="og:title" content="${title}" />
        <meta property="og:description" content="${description}" />
        <meta property="og:image" content="%image%" />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
        <meta property="og:url" content="${url}" />
        <meta property="og:site_name" content="GitButler" />

        <!-- Twitter Card Meta Tags -->
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content="${title}" />
        <meta name="twitter:description" content="${description}" />
        <meta name="twitter:image" content="%image%" />
        <meta name="twitter:site" content="@gitbutler" />
        <!-- / Meta Tags -->
`;

	const regex_patch = /\/([^/]+)\/([^/]+)\/reviews\/([^/]+)\/commit\/([^/]+)/;
	let match = url.match(regex_patch);
	if (match) {
		const [_, user, project, reviewId, changeId] = match;
		metaTags = metaTags.replaceAll(
			"%image%",
			`${env.PUBLIC_APP_HOST}og/review/${user}/${project}/${reviewId}/${changeId}`,
		);

		try {
			const response = await fetch(
				env.PUBLIC_APP_HOST +
					`api/patch_stack/${user}/${project}/branch/${reviewId}/commit/${changeId}`,
			);
			const data = await response.json();
			metaTags = replacePropertyContent(metaTags, "title", `Review ${data.title}`);
			if (data.description) {
				metaTags = replacePropertyContent(metaTags, "description", data.description);
			} else {
				metaTags = replacePropertyContent(
					metaTags,
					"description",
					`Review code for ${user}/${project}`,
				);
			}
			return html.replace("%metatags%", metaTags);
		} catch (error: unknown) {
			console.error("Fetch error:", error);
			return html.replace("%metatags%", metaTags);
		}
	}

	const regex_review = /\/([^/]+)\/([^/]+)\/reviews\/([^/]+)/;
	match = url.match(regex_review);
	if (match) {
		const [_, user, project, reviewId] = match;
		metaTags = metaTags.replaceAll(
			"%image%",
			`${env.PUBLIC_APP_HOST}og/review/${user}/${project}/${reviewId}`,
		);

		// hit the API for this patch and get the project name
		try {
			const response = await fetch(
				env.PUBLIC_APP_HOST + `api/patch_stack/${user}/${project}/branch/${reviewId}`,
			);
			const data = await response.json();
			metaTags = replacePropertyContent(metaTags, "title", `Review ${data.title}`);
			if (data.description) {
				metaTags = replacePropertyContent(metaTags, "description", data.description);
			} else {
				metaTags = replacePropertyContent(
					metaTags,
					"description",
					`Review code for ${user}/${project}`,
				);
			}
			return html.replace("%metatags%", metaTags);
		} catch (error: unknown) {
			console.error("Fetch error:", error);
			return html.replace("%metatags%", metaTags);
		}
	}

	// Default fallback for non-review pages
	metaTags = metaTags.replaceAll("%image%", `${env.PUBLIC_CLOUD_HOST}og-image.png`);
	return html.replace("%metatags%", metaTags);
}
