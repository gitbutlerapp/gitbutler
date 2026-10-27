import { fetchAndProcessReleases, createLatestReleaseBuilds } from "$lib/utils/releaseUtils";
import type { PageLoad } from "./$types";

// Render on the server so crawlers see the page text and links.
export const ssr = true;

// eslint-disable-next-line func-style
export const load: PageLoad = async () => {
	const nightlies = await fetchAndProcessReleases(15, "nightly");
	const latestNightly = nightlies[0] || null;
	const latestNightlyBuilds = latestNightly ? createLatestReleaseBuilds(latestNightly) : {};
	const otherNightlies = nightlies.slice(1);

	return {
		otherNightlies,
		latestNightly,
		latestNightlyBuilds,
	};
};
