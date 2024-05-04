import { normalizeBranchName } from '$lib/utils/branch';
import { describe, expect, test } from 'vitest';

describe.concurrent('normalizeBranchName', () => {
	test('it should remove undesirable symbols', () => {
		expect(normalizeBranchName('a£^&*() b')).toEqual('a-b');
	});

	test('it should preserve capital letters', () => {
		expect(normalizeBranchName('Hello World')).toEqual('Hello-World');
	});

	test('it should preserve `#`, `_`, `/`, and `.`', () => {
		expect(normalizeBranchName('hello#_./world')).toEqual('hello#_./world');
	});
});
