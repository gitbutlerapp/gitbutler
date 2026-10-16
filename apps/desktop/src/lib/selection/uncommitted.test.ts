import { createUncommittedSlice, uncommittedSelectors } from "$lib/selection/uncommitted";
import { describe, expect, test } from "vitest";
import type { HunkAssignment, HunkHeader, TreeChange } from "@gitbutler/but-sdk";

const path = "file.txt";
const hunkHeader: HunkHeader = { oldStart: 1, oldLines: 1, newStart: 1, newLines: 2 };
const change = { path, pathBytes: [], status: { type: "Modification" } } as unknown as TreeChange;
const assignment = {
	id: null,
	hunkHeader,
	path,
	pathBytes: [],
	stackId: null,
	branchRefBytes: null,
	lineNumsAdded: [1, 2],
	lineNumsRemoved: [1],
} as unknown as HunkAssignment;

describe("a linked worktree's uncommitted slice", () => {
	const main = createUncommittedSlice("uncommitted");
	const worktree = createUncommittedSlice("uncommitted:wt");
	const update = worktree.actions.update({ changes: [change], assignments: [assignment] });
	const checkFile = worktree.actions.checkFile({ stackId: null, path });

	test("selects in its own state", () => {
		const state = [update, checkFile].reduce(worktree.reducer, worktree.getInitialState());
		const status = uncommittedSelectors.hunkSelection.fileCheckStatus(state, {
			stackId: null,
			path,
		});
		expect(status).toBe("checked");
	});

	test("leaves the main checkout's state alone", () => {
		const state = [update, checkFile].reduce(main.reducer, main.getInitialState());
		expect(uncommittedSelectors.treeChanges.selectIds(state.treeChanges)).toEqual([]);
	});
});
