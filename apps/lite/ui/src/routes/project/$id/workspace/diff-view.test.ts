import { uncommittedChangesFileParent } from "#ui/addresses.ts";
import type { DiffLineSelection } from "#ui/cursors.ts";
import { lineSelectionsForRange } from "#ui/hunk.ts";
import { getDiffView, prepareDiffFiles, resolveDiffSelection } from "./diff-view.ts";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";

const change: TreeChange = {
	path: "file.ts",
	pathBytes: [],
	status: {
		type: "Modification",
		subject: {
			previousState: { id: "old", kind: "Blob" },
			state: { id: "new", kind: "Blob" },
			flags: null,
		},
	},
};
const patch: UnifiedPatch = {
	type: "Patch",
	subject: {
		hunks: [
			{
				oldStart: 1,
				oldLines: 5,
				newStart: 1,
				newLines: 3,
				diff: "@@ -1,5 +1,3 @@\n-a\n-b\n-c\n+d\n context\n-e\n+f\n",
			},
		],
		isResultOfBinaryToTextConversion: false,
		linesAdded: 2,
		linesRemoved: 4,
	},
};
const viewFor = (treeChangeDiff: UnifiedPatch | undefined) =>
	getDiffView(
		prepareDiffFiles({
			fileParent: uncommittedChangesFileParent,
			changes: [change],
			treeChangeDiffs: [treeChangeDiff],
		}),
	);
const firstBlock: DiffLineSelection = {
	file: { parent: uncommittedChangesFileParent, path: change.path },
	range: null,
};

describe("resolveDiffSelection", () => {
	it("resolves file activation after its diff arrives using the destination layout", () => {
		expect(
			resolveDiffSelection({
				selection: firstBlock,
				fileByItemId: viewFor(undefined).fileByItemId,
				diffStyle: "unified",
			}),
		).toBeNull();

		const { fileByItemId } = viewFor(patch);
		for (const diffStyle of ["split", "unified"] as const) {
			const selection = resolveDiffSelection({ selection: firstBlock, fileByItemId, diffStyle });
			expect(selection?.range).toBeTruthy();
			if (!selection) throw new Error("Missing selection");
			const file = fileByItemId.get(selection.id);
			if (!file) throw new Error("Missing file");
			expect(
				lineSelectionsForRange({
					hunks: file.item.fileDiff.hunks,
					range: selection.range,
					diffStyle,
				}).flatMap((selection) => selection.lineGroups),
			).toEqual([
				{ side: "deletions", start: 1, lines: 3 },
				{ side: "additions", start: 1, lines: 1 },
			]);
		}
	});

	it("preserves an explicit line range", () => {
		const selection: DiffLineSelection = {
			...firstBlock,
			range: { start: 2, side: "deletions", end: 3 },
		};
		for (const diffStyle of ["split", "unified"] as const) {
			expect(
				resolveDiffSelection({ selection, fileByItemId: viewFor(patch).fileByItemId, diffStyle })
					?.range,
			).toBe(selection.range);
		}
	});

	it("leaves a file without changed blocks unselected", () => {
		expect(
			resolveDiffSelection({
				selection: firstBlock,
				fileByItemId: viewFor({ type: "Binary" }).fileByItemId,
				diffStyle: "split",
			}),
		).toBeNull();
	});
});
