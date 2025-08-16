import * as BranchUtils from '$lib/utils/branch';
import { expect, test, describe } from 'vitest';

describe.concurrent('getBranchNameFromRef', () => {
	test('When provided a ref with a remote prefix, it returns the branch name', () => {
		const ref = 'refs/remotes/origin/main';
		const remote = 'origin';

		expect(BranchUtils.getBranchNameFromRef(ref, remote)).toBe('main');
	});

	test("When provided a ref with a remote prefix that can't be found, it throws an error", () => {
		const ref = 'main';
		const remote = 'origin';

		expect(() => BranchUtils.getBranchNameFromRef(ref, remote)).toThrowError();
	});

	test('When provided a ref with a remote prefix and multiple separators, it returns the correct branch name', () => {
		const ref = 'refs/remotes/origin/feature/cool-thing';
		const remote = 'origin';

		expect(BranchUtils.getBranchNameFromRef(ref, remote)).toBe('feature/cool-thing');
	});

	test('When provided a ref with a remote that has multiple separators in it, it returns the correct branch name', () => {
		const ref = 'refs/remotes/origin/feature/cool-thing';
		const remote = 'origin/feature';

		expect(BranchUtils.getBranchNameFromRef(ref, remote)).toBe('cool-thing');
	});

	test('When provided a ref but no explicit remote, it returns the branch name with the remote prefix', () => {
		const ref = 'refs/remotes/origin/main';

		expect(BranchUtils.getBranchNameFromRef(ref)).toBe('origin/main');
	});
});

describe.concurrent('getBranchRemoteFromRef', () => {
	test('When provided a ref with a remote prefix, it returns the remote name', () => {
		const ref = 'refs/remotes/origin/main';

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBe('origin');
	});

	test('When provided a ref without a remote prefix, it returns undefined', () => {
		const ref = 'main';

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBeUndefined();
	});

	test('When provided a ref with a remote prefix and multiple separators, it returns the remote name', () => {
		const ref = 'refs/remotes/origin/feature/cool-thing';

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBe('origin');
	});
});

describe.concurrent('parseBranchName', () => {
	test('When provided a local branch name without remote, it returns just the branch name', () => {
		const result = BranchUtils.parseBranchName('feature/awesome-stuff');

		expect(result).toEqual({
			remote: undefined,
			branchName: 'feature/awesome-stuff'
		});
	});

	test('When provided a branch with origin remote prefix, it extracts remote and branch name', () => {
		const result = BranchUtils.parseBranchName('origin/feature/awesome-stuff');

		expect(result).toEqual({
			remote: 'origin',
			branchName: 'feature/awesome-stuff'
		});
	});

	test('When provided a branch with upstream remote prefix, it extracts remote and branch name', () => {
		const result = BranchUtils.parseBranchName('upstream/main');

		expect(result).toEqual({
			remote: 'upstream',
			branchName: 'main'
		});
	});

	test('When provided a branch with fork remote prefix, it extracts remote and branch name', () => {
		const result = BranchUtils.parseBranchName('fork/bugfix/issue-123');

		expect(result).toEqual({
			remote: 'fork',
			branchName: 'bugfix/issue-123'
		});
	});

	test('When provided a branch with an explicit remote parameter, it uses the provided remote', () => {
		const result = BranchUtils.parseBranchName('origin/feature/test', 'upstream');

		expect(result).toEqual({
			remote: 'upstream',
			branchName: 'feature/test'
		});
	});

	test('When provided a branch name that matches the provided remote, it strips the remote prefix', () => {
		const result = BranchUtils.parseBranchName('origin/feature/test', 'origin');

		expect(result).toEqual({
			remote: 'origin',
			branchName: 'feature/test'
		});
	});

	test('When provided a branch name with non-standard remote name, it treats it as part of branch name', () => {
		const result = BranchUtils.parseBranchName('my-company/special-feature');

		expect(result).toEqual({
			remote: undefined,
			branchName: 'my-company/special-feature'
		});
	});

	test('When provided a single-part branch name, it returns unchanged', () => {
		const result = BranchUtils.parseBranchName('main');

		expect(result).toEqual({
			remote: undefined,
			branchName: 'main'
		});
	});

	test('When provided an empty string, it throws an error', () => {
		expect(() => BranchUtils.parseBranchName('')).toThrow();
	});

	test('When provided a branch with multiple slashes but unknown remote, it keeps the full path', () => {
		const result = BranchUtils.parseBranchName('some/deep/feature/branch');

		expect(result).toEqual({
			remote: undefined,
			branchName: 'some/deep/feature/branch'
		});
	});

	test('When provided a branch with remote remote prefix, it extracts remote and branch name', () => {
		const result = BranchUtils.parseBranchName('remote/hotfix/critical-bug');

		expect(result).toEqual({
			remote: 'remote',
			branchName: 'hotfix/critical-bug'
		});
	});

	test('When provided a complex nested branch with known remote, it handles correctly', () => {
		const result = BranchUtils.parseBranchName('origin/release/v2.1.0/hotfix/security');

		expect(result).toEqual({
			remote: 'origin',
			branchName: 'release/v2.1.0/hotfix/security'
		});
	});
});
