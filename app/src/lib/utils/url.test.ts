import { remoteUrlIsHttp } from '$lib/utils/url';
import { describe, expect, test } from 'vitest';

describe.concurrent('cleanUrl', () => {
	const httpRemoteUrls = ['https://github.com/user/repo.git', 'http://192.168.1.1/user/repo.git'];

	test.each(httpRemoteUrls)('HTTP Remote - %s', (remoteUrl: string) => {
		expect(remoteUrlIsHttp(remoteUrl)).toBe(true);
	});

	const nonHttpRemoteUrls = [
		'git@github.com:user/repo.git',
		'ssh://git@github.com:22/user/repo.git',
		'git://github.com/user/repo.git'
	];

	test.each(nonHttpRemoteUrls)('Non HTTP Remote - %s', (remoteUrl: string) => {
		expect(remoteUrlIsHttp(remoteUrl)).toBe(false);
	});
});
