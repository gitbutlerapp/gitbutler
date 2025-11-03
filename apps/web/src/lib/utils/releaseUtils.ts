import { getValidReleases, type Build, type Release } from '$lib/types/releases';

const API_BASE_URL = 'https://app.gitbutler.com/api/downloads';

/**
 * Process builds by filtering out .zip files, removing duplicates, and sorting by platform
 */
export function processBuilds(builds: Build[]): Build[] {
	return builds
		.filter((build) => !build.url.endsWith('.zip'))
		.filter((build, index, self) => self.findIndex((b) => b.url === build.url) === index)
		.sort((a, b) => b.platform.localeCompare(a.platform));
}

/**
 * Find a specific build based on OS, architecture, and optional file includes criteria
 */
export function findBuild(
	builds: Build[],
	os: string,
	arch?: 'x86_64' | 'aarch64',
	fileIncludes?: string
): Build | undefined {
	return builds.find(
		(build: Build) =>
			build.os === os &&
			(!arch || build.arch === arch) &&
			(!fileIncludes || build.file.includes(fileIncludes))
	);
}

/**
 * Remove duplicate releases based on version, keeping the first occurrence.
 * This is useful when the API returns duplicate releases with the same version.
 */
export function deduplicateReleases(releases: Release[]): Release[] {
	return releases.filter(
		(release, index, self) => self.findIndex((r) => r.version === release.version) === index
	);
}

/**
 * Create standardized build mapping for the latest release with common platform configurations
 */
export interface LatestReleaseBuilds {
	darwin_x86_64: Build | undefined;
	darwin_aarch64: Build | undefined;
	windows_x86_64: Build | undefined;
	linux_appimage_x86_64: Build | undefined;
	linux_deb_x86_64: Build | undefined;
	linux_rpm_x86_64: Build | undefined;
	linux_appimage_aarch64: Build | undefined;
	linux_deb_aarch64: Build | undefined;
	linux_rpm_aarch64: Build | undefined;
}

export function createLatestReleaseBuilds(latestRelease: Release): LatestReleaseBuilds {
	return {
		darwin_x86_64: findBuild(latestRelease.builds, 'darwin', 'x86_64'),
		darwin_aarch64: findBuild(latestRelease.builds, 'darwin', 'aarch64'),
		windows_x86_64: findBuild(latestRelease.builds, 'windows', 'x86_64'),
		linux_appimage_x86_64: findBuild(latestRelease.builds, 'linux', 'x86_64', 'AppImage'),
		linux_deb_x86_64: findBuild(latestRelease.builds, 'linux', 'x86_64', 'deb'),
		linux_rpm_x86_64: findBuild(latestRelease.builds, 'linux', 'x86_64', 'rpm'),
		linux_appimage_aarch64: findBuild(latestRelease.builds, 'linux', 'aarch64', 'AppImage'),
		linux_deb_aarch64: findBuild(latestRelease.builds, 'linux', 'aarch64', 'deb'),
		linux_rpm_aarch64: findBuild(latestRelease.builds, 'linux', 'aarch64', 'rpm')
	};
}

/**
 * Process all releases by applying processBuilds to each release's builds array and removing duplicates
 */
export function processAllReleases(releases: Release[]): Release[] {
	const processedReleases = releases.map((release) => ({
		...release,
		builds: processBuilds(release.builds)
	}));

	// Remove duplicate releases based on version using the dedicated function
	return deduplicateReleases(processedReleases);
}

/**
 * Fetch releases from the GitButler API
 */
export async function fetchReleases(
	limit: number = 10,
	channel: 'release' | 'nightly' = 'release'
): Promise<Release[]> {
	const response = await fetch(`${API_BASE_URL}?limit=${limit}&channel=${channel}`);
	const data = await response.json();
	return getValidReleases(data);
}

/**
 * Fetch and process releases from the GitButler API
 */
export async function fetchAndProcessReleases(
	limit: number = 10,
	channel: 'release' | 'nightly' = 'release'
): Promise<Release[]> {
	const releases = await fetchReleases(limit, channel);
	return processAllReleases(releases);
}
