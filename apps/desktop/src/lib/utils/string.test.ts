import { slugify } from '$lib/utils/string';
import { describe, expect, test } from 'vitest';

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
