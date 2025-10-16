import { fetchAndProcessReleases, createLatestReleaseBuilds } from '$lib/utils/releaseUtils';
import type { PageLoad } from './$types';

// Enable client-side navigation for this page
export const csr = true;
export const ssr = true;

// Cache for 60 seconds to improve navigation performance
let cachedData: any = null;
let cacheTime = 0;
const CACHE_DURATION = 60 * 1000; // 60 seconds

// eslint-disable-next-line func-style
export const load: PageLoad = async () => {
	// Check if we have cached data that's still valid
	const now = Date.now();
	if (cachedData && now - cacheTime < CACHE_DURATION) {
		return cachedData;
	}

	const releases = await fetchAndProcessReleases(10, 'release');
	const nightlies = await fetchAndProcessReleases(15, 'nightly');
	const latestRelease = releases[0];

	const latestReleaseBuilds = createLatestReleaseBuilds(latestRelease);

	const data = {
		nightlies,
		releases,
		latestRelease,
		latestReleaseBuilds
	};

	// Cache the data
	cachedData = data;
	cacheTime = now;

	return data;
};
