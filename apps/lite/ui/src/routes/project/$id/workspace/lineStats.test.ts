import { assert } from "#ui/assert.ts";
import { getLineStats, patchLineStats } from "#ui/routes/project/$id/workspace/lineStats.ts";
import {
	parsePreparedDiffFile,
	prepareDiffFiles,
} from "#ui/routes/project/$id/workspace/diff-view.ts";
import { uncommittedChangesFileParent } from "#ui/addresses.ts";
import type { ChangeState, DiffHunk, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";

const state = (id: string): ChangeState => ({ id, kind: "Blob" });

const modification = (path: string): TreeChange => ({
	path,
	pathBytes: [],
	status: {
		type: "Modification",
		subject: { previousState: state("a"), state: state("b"), flags: null },
	},
});

/** One changed line in a three-line file — the rest of the hunk is context. */
const oneLineEdit: DiffHunk = {
	oldStart: 1,
	oldLines: 3,
	newStart: 1,
	newLines: 3,
	diff: ["@@ -1,3 +1,3 @@", " one", "-two", "+TWO", " three", ""].join("\n"),
};

const patch = (hunks: Array<DiffHunk>, linesAdded: number, linesRemoved: number): UnifiedPatch => ({
	type: "Patch",
	subject: { hunks, isResultOfBinaryToTextConversion: false, linesAdded, linesRemoved },
});

describe("patchLineStats", () => {
	it("reports the change deltas, not the size of each side of the patch", () => {
		const change = modification("file.ts");
		const unified = patch([oneLineEdit], 1, 1);

		expect(patchLineStats(unified)).toEqual({ linesAdded: 1, linesRemoved: 1 });

		// The parsed patch's line arrays hold every line present on each side,
		// context included, so their lengths overcount a one-line edit — a header
		// that reads them renders "+3 -3" for this hunk.
		const prepared = prepareDiffFiles({
			fileParent: uncommittedChangesFileParent,
			changes: [change],
			treeChangeDiffs: [unified],
		});
		const fileDiff = parsePreparedDiffFile(assert(prepared[0]));
		expect(fileDiff.additionLines).toHaveLength(3);
		expect(fileDiff.deletionLines).toHaveLength(3);
	});

	it("has no counts for a change without a patch", () => {
		expect(patchLineStats({ type: "Binary" })).toBeNull();
		expect(patchLineStats({ type: "TooLarge", subject: { sizeInBytes: 1e9 } })).toBeNull();
		expect(patchLineStats(null)).toBeNull();
		expect(patchLineStats(undefined)).toBeNull();
	});
});

describe("getLineStats", () => {
	it("totals the counted changes and skips the uncountable ones", () => {
		expect(
			getLineStats([patch([oneLineEdit], 1, 1), { type: "Binary" }, null, patch([], 4, 2)]),
		).toEqual({ linesAdded: 5, linesRemoved: 3 });
	});
});
