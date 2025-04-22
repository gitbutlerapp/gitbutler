import { captureMessage } from '@sentry/sveltekit';

export interface Build {
	os: 'windows' | 'darwin' | 'linux';
	arch: 'x86_64' | 'aarch64';
	url: string;
	file: string;
	platform: string;
}

export function isReleaseBuild(something: unknown): something is Build {
	return (
		typeof something === 'object' &&
		something !== null &&
		typeof (something as any).os === 'string' &&
		typeof (something as any).arch === 'string' &&
		typeof (something as any).url === 'string' &&
		typeof (something as any).file === 'string' &&
		typeof (something as any).platform === 'string'
	);
}

export interface Release {
	version: string;
	notes: string | null;
	sha: string;
	channel: 'release' | string;
	build_version: string;
	released_at: string;
	builds: Build[];
}

export function isRelease(something: unknown): something is Release {
	return (
		typeof something === 'object' &&
		something !== null &&
		typeof (something as any).version === 'string' &&
		(typeof (something as any).notes === 'string' || (something as any).notes === null) &&
		typeof (something as any).sha === 'string' &&
		typeof (something as any).channel === 'string' &&
		typeof (something as any).build_version === 'string' &&
		typeof (something as any).released_at === 'string' &&
		Array.isArray((something as any).builds) &&
		(something as any).builds.every(isReleaseBuild)
	);
}

export function getValidReleases(something: unknown): Release[] {
	if (!Array.isArray(something)) return [];
	const result: Release[] = [];
	for (const item of something) {
		if (isRelease(item)) {
			result.push(item);
			continue;
		}

		captureMessage('Invalid release filtered out', {
			level: 'warning',
			extra: {
				release: item
			}
		});
	}
	return result;
}
