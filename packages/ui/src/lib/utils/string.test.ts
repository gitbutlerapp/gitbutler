import { slugify, camelCaseToTitleCase } from '$lib/utils/string';
import { describe, expect, test } from 'vitest';

describe.concurrent('camelCaseToTitleCase with valid inputs', () => {
	test('localAndRemote', () => {
		expect(camelCaseToTitleCase('localAndRemote')).toEqual('Local And Remote');
	});

	test('local', () => {
		expect(camelCaseToTitleCase('local')).toEqual('Local');
	});

	test('remote', () => {
		expect(camelCaseToTitleCase('remote')).toEqual('Remote');
	});

	test('localAndShadow', () => {
		expect(camelCaseToTitleCase('localAndShadow')).toEqual('Local And Shadow');
	});

	test('LocalAndShadow', () => {
		expect(camelCaseToTitleCase('LocalAndShadow')).toEqual('Local And Shadow');
	});
});

describe.concurrent('branch slugify with valid characters', () => {
	test('forward slashes are fine', () => {
		expect(slugify('my/branch')).toEqual('my/branch');
	});

	test('capitalization is fine', () => {
		expect(slugify('MY/branch')).toEqual('MY/branch');
	});

	test('numbers are fine', () => {
		expect(slugify('my/branch1')).toEqual('my/branch1');
	});

	test('underscores are fine', () => {
		expect(slugify('my_branch')).toEqual('my_branch');
	});
});

describe.concurrent('branch slugify with replaced characters', () => {
	test('whitespaces are truncated', () => {
		expect(slugify(' my/branch ')).toEqual('my/branch');
	});

	test('whitespace in the middle becomes dash', () => {
		expect(slugify('my branch')).toEqual('my-branch');
	});

	test('most special characters are nuked', () => {
		expect(slugify('a!b@c$d;e%f^g&h*i(j)k+l=m~n`')).toEqual('abcdefghijklmn');
	});
});
