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
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { type CodeViewDiffItem, type CodeViewLineSelection, parsePatchFiles } from "@pierre/diffs";

export type Annotation = { _tag: "local"; id: string };

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
};

export type DiffView = {
	navigationIndex: NavigationIndex<HunkOperand>;
	items: Array<CodeViewDiffItem<Annotation>>;
	fileByItemId: Map<string, DiffViewFile>;
	fileByPath: Map<string, DiffViewFile>;
	fileByHunkKey: Map<string, DiffViewFile>;
	hunkByKey: Map<string, DiffViewHunk>;
};

export const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

const parseFileDiff = (
	patch: string,
	version: string,
): CodeViewDiffItem<Annotation>["fileDiff"] => {
	const parsed = parsePatchFiles(patch, version);

	const [parsedPatch, ...restPatches] = parsed;
	if (!parsedPatch) throw new Error("Failed to parse any patches");
	if (restPatches.length > 0) throw new Error("Parsed more than one patch");

	const [fileDiff, ...restFiles] = parsedPatch.files;
	if (!fileDiff) throw new Error("Failed to parse any files in patch");
	if (restFiles.length > 0) throw new Error("Parsed more than one file in patch");

	return fileDiff;
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

	if (treeChangeDiff?.type !== "Patch") return { itemId, firstHunk: null };

	for (const diffHunk of treeChangeDiff.subject.hunks) {
		const patch = synthesizeFilePatch(change, [diffHunk]);
		const fileDiff = parseFileDiff(patch, itemId);

		for (const hunk of fileDiff.hunks) {
			const [selection] = contiguousSelectionsFromHunk(hunk);
			if (!selection) continue;

			return {
				itemId,
				firstHunk: {
					parent: file,
					...selection,
					isResultOfBinaryToTextConversion: treeChangeDiff.subject.isResultOfBinaryToTextConversion,
				},
			};
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
	const fileByHunkKey = new Map<string, DiffViewFile>();
	const hunkByKey = new Map<string, DiffViewHunk>();

	for (const [ci, change] of changes.entries()) {
		const mdiff = treeChangeDiffs[ci];

		const file: FileOperand = {
			parent: fileParent,
			path: change.path,
		};

		const combinedFilePatch = synthesizeFilePatch(
			change,
			mdiff && "subject" in mdiff && "hunks" in mdiff.subject ? mdiff.subject.hunks : [],
		);
		const version = hash(combinedFilePatch);
		const item: CodeViewDiffItem<Annotation> = {
			type: "diff",
			id: weakFileIdentityKey(file),
			version,
			fileDiff: parseFileDiff(combinedFilePatch, String(version)),
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
					};
					diffViewFile.hunks.push(diffViewHunk);
					fileByHunkKey.set(hunkKey, diffViewFile);
					hunkByKey.set(hunkKey, diffViewHunk);
				}
			}
		}
	}

	return {
		items,
		fileByItemId,
		fileByPath,
		fileByHunkKey,
		hunkByKey,
		navigationIndex,
	};
};
