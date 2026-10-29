import * as BranchUtils from "$lib/branches/branchUtils";
import { expect, test, describe } from "vitest";

describe.concurrent("getBranchRemoteFromRef", () => {
	test("When provided a ref with a remote prefix, it returns the remote name", () => {
		const ref = "refs/remotes/origin/main";

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBe("origin");
	});

	test("When provided a ref without a remote prefix, it returns undefined", () => {
		const ref = "main";

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBeUndefined();
	});

	test("When provided a ref with a remote prefix and multiple separators, it returns the remote name", () => {
		const ref = "refs/remotes/origin/feature/cool-thing";

		expect(BranchUtils.getBranchRemoteFromRef(ref)).toBe("origin");
	});
});
