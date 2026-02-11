import { slugify, slugifyBranchName, camelCaseToTitleCase } from '$lib/utils/string';
import { describe, expect, test } from 'vitest';

describe.concurrent('camelCaseToTitleCase with valid inputs', () => {
	test('localAndRemote', () => {
		expect(camelCaseToTitleCase('localAndRemote')).toEqual('Local And Remote');
	});

	test('local', () => {
		expect(camelCaseToTitleCase('localOnly')).toEqual('Local Only');
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

	test('periods are fine', () => {
		expect(slugify('my.branch')).toEqual('my.branch');
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

describe.concurrent('slugifyBranchName preserves git-valid characters', () => {
	test('hash/number sign is preserved', () => {
		expect(slugifyBranchName('feat/#1234-ticket-name')).toEqual('feat/#1234-ticket-name');
	});

	test('at sign is preserved', () => {
		expect(slugifyBranchName('user@feature')).toEqual('user@feature');
	});

	test('forward slashes are preserved', () => {
		expect(slugifyBranchName('my/branch')).toEqual('my/branch');
	});

	test('periods are preserved', () => {
		expect(slugifyBranchName('v1.2.3')).toEqual('v1.2.3');
	});

	test('whitespace becomes hyphens', () => {
		expect(slugifyBranchName('my branch name')).toEqual('my-branch-name');
	});

	test('git-invalid characters are removed', () => {
		expect(slugifyBranchName('a~b^c:d?e*f')).toEqual('abcdef');
	});

	test('behaves like slugify for simple names', () => {
		expect(slugifyBranchName('feature/my-branch')).toEqual('feature/my-branch');
	});
});
