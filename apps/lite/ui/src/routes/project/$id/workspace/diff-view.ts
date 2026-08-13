import { assert } from "#ui/assert.ts";
import { hash } from "#ui/hash.ts";
import {
	contiguousSelectionsFromHunk,
	rangeFromLineGroups,
	synthesizeFilePatch,
} from "#ui/hunk.ts";
import {
	hunkOperand,
	operandIdentityKey,
	type FileOperand,
	type FileParent,
	type HunkOperand,
	weakFileIdentityKey,
} from "#ui/operands.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import {
	processFile,
	type CodeViewDiffItem,
	type CodeViewLayout,
	type CodeViewLineSelection,
	type VirtualFileMetrics,
} from "@pierre/diffs";

export type Annotation = { _tag: "local"; id: string };

/**
 * Layout and metrics handed to CodeView. Shared because the minimap models item
 * positions from the same numbers, and would drift silently if they diverged.
 */
export const codeViewLayout: CodeViewLayout = {
	paddingTop: 0,
	// Match --panel-padding-block.
	paddingBottom: 12,
	gap: 10,
};

export const codeViewItemMetrics = {
	diffHeaderHeight: 38,
	paddingBottom: 9,
} satisfies Partial<VirtualFileMetrics>;

type PrepareDiffFilesDeps = {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
};

export type PreparedDiffFile = {
	file: FileOperand;
	fileId: string;
	change: TreeChange;
	treeChangeDiff: UnifiedPatch | null;
	patch: string;
	version: number;
};

export type DiffViewFile = {
	operand: FileOperand;
	item: CodeViewDiffItem<Annotation>;
	change: TreeChange;
	patch: UnifiedPatch | null;
	hunks: Array<DiffViewHunk>;
};

type DiffViewHunk = {
	operand: HunkOperand;
	selectedLines: CodeViewLineSelection;
	file: DiffViewFile;
};

export type DiffView = {
	navigationIndex: NavigationIndex<HunkOperand>;
	items: Array<CodeViewDiffItem<Annotation>>;
	fileByItemId: Map<string, DiffViewFile>;
	fileByPath: Map<string, DiffViewFile>;
	hunkByKey: Map<string, DiffViewHunk>;
};

export const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

const parseFileDiff = (
	patch: string,
	version: string,
): CodeViewDiffItem<Annotation>["fileDiff"] => {
	const parsed = processFile(patch, { cacheKey: version });
	if (!parsed) throw new Error("Failed to parse patch");

	return parsed;
};

/**
 * Prepare stable file IDs and patch versions once for both review state and
 * the rendered diff. Parsing remains separate so single-file mode only builds
 * Pierre's diff shape for the selected file.
 */
export const prepareDiffFiles = ({
	fileParent,
	changes,
	treeChangeDiffs,
}: PrepareDiffFilesDeps): Array<PreparedDiffFile> =>
	changes.map((change, index) => {
		const file: FileOperand = { parent: fileParent, path: change.path };
		const treeChangeDiff = treeChangeDiffs[index] ?? null;
		const patch = synthesizeFilePatch(
			change,
			treeChangeDiff?.type === "Patch" ? treeChangeDiff.subject.hunks : [],
		);

		return {
			file,
			fileId: weakFileIdentityKey(file),
			change,
			treeChangeDiff,
			patch,
			version: hash(patch),
		};
	});

export const parsePreparedDiffFile = (
	file: PreparedDiffFile,
): CodeViewDiffItem<Annotation>["fileDiff"] => parseFileDiff(file.patch, String(file.version));

type DiffFileNavigation = {
	itemId: string;
	firstHunk: HunkOperand | null;
};

export const getDiffFileNavigation = ({
	fileParent,
	change,
	treeChangeDiff,
}: {
	fileParent: FileParent;
	change: TreeChange;
	treeChangeDiff: UnifiedPatch | null;
}): DiffFileNavigation => {
	const file: FileOperand = {
		parent: fileParent,
		path: change.path,
	};
	const itemId = weakFileIdentityKey(file);

	if (treeChangeDiff?.type === "Patch") {
		const fstDiffHunk = treeChangeDiff.subject.hunks[0];
		if (fstDiffHunk) {
			const fstHunk = parseFileDiff(synthesizeFilePatch(change, [fstDiffHunk]), itemId).hunks[0];
			if (fstHunk) {
				const fstSelection = contiguousSelectionsFromHunk(fstHunk).next().value;
				if (fstSelection) {
					return {
						itemId,
						firstHunk: {
							parent: file,
							...fstSelection,
							isResultOfBinaryToTextConversion:
								treeChangeDiff.subject.isResultOfBinaryToTextConversion,
						},
					};
				}
			}
		}
	}

	return { itemId, firstHunk: null };
};

/** Build relationships between our SDK data and Pierre's view. */
export const getDiffView = (files: Array<PreparedDiffFile>): DiffView => {
	const navigationIndex: NavigationIndex<HunkOperand> = {
		items: [],
		indexByKey: new Map(),
	};

	const items: Array<CodeViewDiffItem<Annotation>> = [];

	const fileByItemId = new Map<string, DiffViewFile>();
	const fileByPath = new Map<string, DiffViewFile>();
	const hunkByKey = new Map<string, DiffViewHunk>();

	for (const prepared of files) {
		const { file, fileId, change, treeChangeDiff: mdiff, version } = prepared;
		const item: CodeViewDiffItem<Annotation> = {
			type: "diff",
			id: fileId,
			version,
			fileDiff: parsePreparedDiffFile(prepared),
		};

		items.push(item);

		const diffViewFile: DiffViewFile = {
			operand: file,
			item,
			change,
			patch: mdiff,
			hunks: [],
		};

		fileByItemId.set(item.id, diffViewFile);
		fileByPath.set(change.path, diffViewFile);

		if (mdiff?.type === "Patch") {
			for (const hunk of item.fileDiff.hunks) {
				for (const selection of contiguousSelectionsFromHunk(hunk)) {
					const range = rangeFromLineGroups(selection.lineGroups);
					if (!range) continue;

					const hunkOperand: HunkOperand = {
						parent: file,
						...selection,
						isResultOfBinaryToTextConversion: mdiff.subject.isResultOfBinaryToTextConversion,
					};
					const hunkKey = hunkOperandIdentityKey(hunkOperand);

					const len = navigationIndex.items.push(hunkOperand);
					navigationIndex.indexByKey.set(hunkKey, len - 1);

					const diffViewHunk: DiffViewHunk = {
						operand: hunkOperand,
						selectedLines: {
							id: item.id,
							range,
						},
						file: diffViewFile,
					};
					diffViewFile.hunks.push(diffViewHunk);
					hunkByKey.set(hunkKey, diffViewHunk);
				}
			}
		}
	}

	return {
		items,
		fileByItemId,
		fileByPath,
		hunkByKey,
		navigationIndex,
	};
};

/**
 * The navigation index with folded files' hunks removed — except each folded
 * file's first hunk, which stands in for the file the way a folded branch
 * keeps its branch row. j/k then stop once per folded file instead of walking
 * its hidden hunks, and z can unfold from the keyboard.
 */
export const withoutFoldedHunks = (
	navigationIndex: NavigationIndex<HunkOperand>,
	hunkByKey: DiffView["hunkByKey"],
	collapsedItems: Set<string>,
): NavigationIndex<HunkOperand> => {
	if (collapsedItems.size === 0) return navigationIndex;

	const items = navigationIndex.items.filter((hunk) => {
		const key = hunkOperandIdentityKey(hunk);
		const file = hunkByKey.get(key)?.file;
		return (
			file === undefined ||
			!collapsedItems.has(file.item.id) ||
			hunkOperandIdentityKey(assert(file.hunks[0]).operand) === key
		);
	});
	return { items, indexByKey: buildIndexByKey(items, hunkOperandIdentityKey) };
};
