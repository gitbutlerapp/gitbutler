import { getValidReleases, type Build, type Release } from '$lib/types/releases';
import type { PageLoad } from './$types';

function processBuilds(builds: Build[]) {
	return builds
		.filter((build) => !build.url.endsWith('.zip'))
		.filter((build, index, self) => self.findIndex((b) => b.url === build.url) === index)
		.sort((a, b) => b.platform.localeCompare(a.platform));
}

function findBuild(builds: Build[], os: string, arch?: string, fileIncludes?: string) {
	return builds.find(
		(build: any) =>
			build.os === os &&
			(!arch || build.arch === arch) &&
			(!fileIncludes || build.file.includes(fileIncludes))
	);
}

// eslint-disable-next-line func-style
export const load: PageLoad = async () => {
	let releases: Release[] = [];
	let nightlies: Release[] = [];
	let latestReleaseBuilds: { [key: string]: Build | unknown } = {};

	const releaseResponse = await fetch(
		'https://app.gitbutler.com/api/downloads?limit=10&channel=release'
	);
	releases = getValidReleases(await releaseResponse.json());
	const latestRelease = releases[0];

	releases.forEach((release) => {
		release.builds = processBuilds(release.builds);
	});

	latestReleaseBuilds = {
		darwin_x86_64: findBuild(latestRelease.builds, 'darwin', 'x86_64'),
		darwin_aarch64: findBuild(latestRelease.builds, 'darwin', 'aarch64'),
		windows_x86_64: findBuild(latestRelease.builds, 'windows', 'x86_64'),
		linux_appimage: findBuild(latestRelease.builds, 'linux', undefined, 'AppImage'),
		linux_deb: findBuild(latestRelease.builds, 'linux', undefined, 'deb'),
		linux_rpm: findBuild(latestRelease.builds, 'linux', undefined, 'rpm')
	};

	const nightlyResponse = await fetch(
		'https://app.gitbutler.com/api/downloads?limit=15&channel=nightly'
	);
	nightlies = getValidReleases(await nightlyResponse.json());
	nightlies.forEach((nightlyRelease) => {
		nightlyRelease.builds = processBuilds(nightlyRelease.builds);
	});

	return {
		nightlies,
		releases,
		latestRelease,
		latestReleaseBuilds
	};
};
