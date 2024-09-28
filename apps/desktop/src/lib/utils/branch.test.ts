import * as BranchUtils from './branch';
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
