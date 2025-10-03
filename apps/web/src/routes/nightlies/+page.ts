import { fetchAndProcessReleases, createLatestReleaseBuilds } from '$lib/utils/releaseUtils';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad = async () => {
	const nightlies = await fetchAndProcessReleases(15, 'nightly');
	const latestNightly = nightlies[0] || null;
	const latestNightlyBuilds = latestNightly ? createLatestReleaseBuilds(latestNightly) : {};

	return {
		nightlies,
		latestNightly,
		latestNightlyBuilds
	};
};
