import { remoteUrlIsHttp, convertRemoteToWebUrl } from '$lib/utils/url';
import { describe, expect, test } from 'vitest';

describe.concurrent('cleanUrl', () => {
	test('it should handle url starts with http(s)?', () => {
		expect(convertRemoteToWebUrl('https://github.com/user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});

	test('it should handle complete ssh url with domain name', () => {
		expect(convertRemoteToWebUrl('ssh://git@github.com:22/user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});

	test('it should handle complete ssh url with ip', () => {
		expect(convertRemoteToWebUrl('ssh://git@192.168.1.1:22/user/repo.git')).toEqual(
			'http://192.168.1.1/user/repo'
		);
	});

	test('it should handle SCP-like url', () => {
		expect(convertRemoteToWebUrl('git@github.com:user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});

	const httpRemoteUrls = ['https://github.com/user/repo.git', 'http://192.168.1.1/user/repo.git'];

	test.each(httpRemoteUrls)('HTTP Remote - %s', (remoteUrl) => {
		expect(remoteUrlIsHttp(remoteUrl)).toBe(true);
	});

	const nonHttpRemoteUrls = [
		'git@github.com:user/repo.git',
		'ssh://git@github.com:22/user/repo.git',
		'git://github.com/user/repo.git'
	];

	test.each(nonHttpRemoteUrls)('Non HTTP Remote - %s', (remoteUrl) => {
		expect(remoteUrlIsHttp(remoteUrl)).toBe(false);
	});
});
