import { BaseBranch } from '$lib/baseBranch/baseBranch';
import { describe, expect, test } from 'vitest';

describe('BaseBranch.shortName', () => {
	const testCases = [
		// Cases with remoteName
		{
			branchName: 'refs/remotes/origin/feature/foo',
			remoteName: 'origin',
			expected: 'feature/foo',
			description: 'full ref with remote'
		},
		{
			branchName: 'origin/feature/foo',
			remoteName: 'origin',
			expected: 'feature/foo',
			description: 'short remote ref with remote'
		},
		{
			branchName: 'refs/remotes/origin/main',
			remoteName: 'origin',
			expected: 'main',
			description: 'full ref with remote, simple name'
		},
		{
			branchName: 'origin/main',
			remoteName: 'origin',
			expected: 'main',
			description: 'short remote ref with remote, simple name'
		},
		{
			branchName: 'refs/remotes/another-remote/feat/complex-branch-name',
			remoteName: 'another-remote',
			expected: 'feat/complex-branch-name',
			description: 'full ref with different remote and complex name'
		},
		{
			branchName: 'another-remote/feat/complex-branch-name',
			remoteName: 'another-remote',
			expected: 'feat/complex-branch-name',
			description: 'short ref with different remote and complex name'
		},

		// Cases with remoteName that does not match the prefix (should not strip remote part)
		{
			branchName: 'refs/remotes/origin/feature/foo',
			remoteName: 'not-origin',
			expected: 'refs/remotes/origin/feature/foo',
			description: 'full ref with non-matching remoteName'
		},
		{
			branchName: 'origin/feature/foo',
			remoteName: 'not-origin',
			expected: 'origin/feature/foo',
			description: 'short ref with non-matching remoteName'
		},

		// Cases where remoteName is set, but branchName is a local or heads ref (remoteName should be ignored for stripping these prefixes)
		{
			branchName: 'refs/heads/feature/foo',
			remoteName: 'origin',
			expected: 'feature/foo',
			description: 'heads ref with remoteName set'
		},
		{
			branchName: 'feature/foo',
			remoteName: 'origin',
			expected: 'feature/foo',
			description: 'local-like name with remoteName set'
		},

		// Cases without remoteName (or remoteName is undefined)
		{
			branchName: 'refs/heads/feature/foo',
			remoteName: undefined,
			expected: 'feature/foo',
			description: 'heads ref, no remoteName'
		},
		{
			branchName: 'feature/foo',
			remoteName: undefined,
			expected: 'feature/foo',
			description: 'local-like name, no remoteName'
		},
		{
			branchName: 'refs/heads/main',
			remoteName: undefined,
			expected: 'main',
			description: 'heads ref, simple name, no remoteName'
		},
		{
			branchName: 'main',
			remoteName: undefined,
			expected: 'main',
			description: 'local-like simple name, no remoteName'
		},
		{
			branchName: 'dev/task/T-123',
			remoteName: undefined,
			expected: 'dev/task/T-123',
			description: 'complex local name, no remoteName'
		},
		{
			branchName: 'refs/heads/dev/task/T-123',
			remoteName: undefined,
			expected: 'dev/task/T-123',
			description: 'complex heads name, no remoteName'
		},

		// Edge cases
		{
			branchName: 'origin',
			remoteName: 'origin',
			expected: '',
			description: 'branchName is just remoteName, prefixed'
		},
		{
			branchName: 'refs/remotes/origin/',
			remoteName: 'origin',
			expected: '',
			description: 'branchName is full remote prefix'
		},
		{
			branchName: 'refs/heads/',
			remoteName: undefined,
			expected: '',
			description: 'branchName is full heads prefix'
		},
		{
			branchName: 'refs/remotes/origin/feature/name-with-refs/heads/in-it',
			remoteName: 'origin',
			expected: 'feature/name-with-refs/heads/in-it',
			description: 'branch name containing other ref parts'
		},
		{
			branchName: '',
			remoteName: 'origin',
			expected: '',
			description: 'empty branch name with remote'
		},
		{
			branchName: '',
			remoteName: undefined,
			expected: '',
			description: 'empty branch name no remote'
		},
		{
			branchName: 'refs/remotes/dev/feature/branch',
			remoteName: 'dev/feature',
			expected: 'branch',
			description: 'remote name with slashes'
		},
		{
			branchName: 'dev/feature/branch',
			remoteName: 'dev/feature',
			expected: 'branch',
			description: 'remote name with slashes, short form'
		}
	];

	for (const { branchName, remoteName, expected, description } of testCases) {
		test(`should return "${expected}" for "${branchName}" (${description})`, () => {
			const baseBranch = new BaseBranch();
			baseBranch.branchName = branchName;
			// Casting because remoteName can be undefined, which is a valid state.
			baseBranch.remoteName = remoteName as string;
			expect(baseBranch.shortName).toBe(expected);
		});
	}

	// Test case for when remoteName is explicitly set but not part of branchName prefixes
	test('should return original name if remoteName is present but no remote prefix in branchName, and no heads prefix', () => {
		const baseBranch = new BaseBranch();
		baseBranch.branchName = 'some/branch/name';
		baseBranch.remoteName = 'origin';
		expect(baseBranch.shortName).toBe('some/branch/name');
	});
});
