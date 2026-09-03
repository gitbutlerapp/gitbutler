import { encodeBytes } from "#ui/api/bytes.ts";
import { createInitialProjectState, projectReducers } from "#ui/projects/project.ts";
import { describe, expect, test } from "vitest";

describe("updateRewrittenBranchReferences", () => {
	const rename = (from: string, to: string, folded: Array<string>) => {
		const state = createInitialProjectState();
		for (const ref of folded) state.workspace.foldedSegments[ref] = true;
		projectReducers.updateRewrittenBranchReferences(state, {
			oldBranch: { branchRef: encodeBytes(from) },
			newBranch: { branchRef: encodeBytes(to) },
		});
		return state.workspace.foldedSegments;
	};

	test("a folded segment stays folded under its new ref after a rename", () => {
		expect(rename("refs/heads/old", "refs/heads/new", ["refs/heads/old"])).toEqual({
			"refs/heads/new": true,
		});
	});

	test("renaming an unfolded segment leaves it unfolded", () => {
		expect(rename("refs/heads/old", "refs/heads/new", [])).toEqual({});
	});

	test("unrelated folded segments are untouched", () => {
		expect(rename("refs/heads/old", "refs/heads/new", ["refs/heads/other"])).toEqual({
			"refs/heads/other": true,
		});
	});
});
