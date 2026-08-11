import { assert } from "#ui/assert.ts";
import { hash } from "#ui/hash.ts";
import {
	contiguousSelectionsFromHunk,
	firstContiguousSelectionFromHunk,
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

export type Annotation =
	| { _tag: "local"; id: string }
	/**
	 * An unresolved conflict of the selected commit, anchored at the line its
	 * region starts in the intended result. `hunk` is 1-based, addressing the
	 * conflict the same way the resolve API does.
	 */
	| { _tag: "conflict"; path: string; hunk: number };

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

type DiffViewDeps = {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
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
 * Parse a change into CodeView's diff shape. Shared with the minimap, which
 * needs the same hunk layout — going through here keeps both on one parse
 * cache entry instead of paying for the patch twice.
 */
export const parseChangeDiff = (
	change: TreeChange,
	patch: UnifiedPatch | null,
): { version: number; fileDiff: CodeViewDiffItem<Annotation>["fileDiff"] } => {
	const combined = synthesizeFilePatch(change, patch?.type === "Patch" ? patch.subject.hunks : []);
	const version = hash(combined);

	return { version, fileDiff: parseFileDiff(combined, String(version)) };
};

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
				const fstSelection = firstContiguousSelectionFromHunk(fstHunk);
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
export const getDiffView = ({ fileParent, changes, treeChangeDiffs }: DiffViewDeps): DiffView => {
	const navigationIndex: NavigationIndex<HunkOperand> = {
		items: [],
		indexByKey: new Map(),
	};

	const items: Array<CodeViewDiffItem<Annotation>> = [];

	const fileByItemId = new Map<string, DiffViewFile>();
	const fileByPath = new Map<string, DiffViewFile>();
	const hunkByKey = new Map<string, DiffViewHunk>();

	for (const [ci, change] of changes.entries()) {
		const mdiff = treeChangeDiffs[ci];

		const file: FileOperand = {
			parent: fileParent,
			path: change.path,
		};

		const { version, fileDiff } = parseChangeDiff(change, mdiff ?? null);
		const item: CodeViewDiffItem<Annotation> = {
			type: "diff",
			id: weakFileIdentityKey(file),
			version,
			fileDiff,
		};

		items.push(item);

		const diffViewFile: DiffViewFile = {
			operand: file,
			item,
			change,
			patch: mdiff ?? null,
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
