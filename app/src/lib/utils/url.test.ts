import { cleanUrl } from '$lib/utils/url';
import { describe, expect, test } from 'vitest';

describe.concurrent('cleanUrl', () => {
	test('it should handle url starts with http(s)?', () => {
		expect(cleanUrl('https://github.com/user/repo.git')).toEqual('https://github.com/user/repo');
	});

	test('it should handle complete ssh url with domain name', () => {
		expect(cleanUrl('ssh://git@github.com:22/user/repo.git')).toEqual('https://github.com/user/repo');
	});

	test('it should handle complete ssh url with ip', () => {
		expect(cleanUrl('ssh://git@192.168.1.1:22/user/repo.git')).toEqual('http://192.168.1.1/user/repo');
	});

	test('it should handle SCP-like url', () => {
		expect(cleanUrl('git@github.com:user/repo.git')).toEqual('https://github.com/user/repo');
	});
});
