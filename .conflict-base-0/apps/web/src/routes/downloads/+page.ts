import { fetchAndProcessReleases, createLatestReleaseBuilds } from '$lib/utils/releaseUtils';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad = async () => {
	const releases = await fetchAndProcessReleases(10, 'release');
	const nightlies = await fetchAndProcessReleases(15, 'nightly');
	const latestRelease = releases[0];

	const latestReleaseBuilds = createLatestReleaseBuilds(latestRelease);

	return {
		nightlies,
		releases,
		latestRelease,
		latestReleaseBuilds
	};
};
