import { encodeBytes } from "#ui/api/bytes.ts";
import { uncommittedChangesFileParent } from "#ui/addresses.ts";
import type { DiffLineSelection } from "#ui/cursors.ts";
import { createInitialProjectState, projectReducers } from "#ui/projects/project.ts";
import { describe, expect, test } from "vitest";

test("activating the same file resets its explicit range to the first changed block", () => {
	const state = createInitialProjectState();
	const selection: DiffLineSelection = {
		file: { parent: uncommittedChangesFileParent, path: "file.ts" },
		range: { start: 3, end: 4, side: "deletions" },
	};
	projectReducers.selectDiffCursor(state, { selection });
	const fileSelection: DiffLineSelection = { ...selection, range: null };
	projectReducers.selectDiffCursor(state, { selection: fileSelection });
	projectReducers.selectDiffCursor(state, { selection: { ...fileSelection } });
	expect(state.workspace.diffCursor).toBe(fileSelection);
});

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
