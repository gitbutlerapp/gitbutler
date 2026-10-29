import { toCommitMovePlacement } from "$lib/stacks/commitMovePlacement";
import { describe, expect, test } from "vitest";

describe("toCommitMovePlacement", () => {
	test("places a commit at the top of a destination stack", () => {
		expect(
			toCommitMovePlacement({
				targetBranchName: "feature/target",
				targetCommitId: "top",
			}),
		).toEqual({
			relativeTo: {
				type: "reference",
				subject: "refs/heads/feature/target",
			},
			side: "below",
		});
	});

	test("places a commit below another commit within a stack", () => {
		expect(
			toCommitMovePlacement({
				targetBranchName: "feature/target",
				targetCommitId: "commit-2",
			}),
		).toEqual({
			relativeTo: {
				type: "commit",
				subject: "commit-2",
			},
			side: "below",
		});
	});
});
