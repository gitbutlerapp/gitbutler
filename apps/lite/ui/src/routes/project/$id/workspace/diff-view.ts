import { assert } from "#ui/assert.ts";
import { hash } from "#ui/hash.ts";
import {
	contiguousSelectionsFromHunk,
	rangeFromLineGroups,
	synthesizeFilePatch,
} from "#ui/hunk.ts";
import { isRasterImageFile, isSvgFile } from "#ui/file.ts";
import {
	hunkAddress,
	addressIdentityKey,
	type FileAddress,
	type FileParent,
	type HunkAddress,
	weakFileIdentityKey,
} from "#ui/addresses.ts";
import { buildIndexByKey, type AddressSpace } from "#ui/workspace/address-space.ts";
import type { DiffLineSelection } from "#ui/cursors.ts";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import {
	processFile,
	type CodeViewDiffItem,
	type CodeViewFileItem,
	type CodeViewItem,
	type CodeViewLayout,
	type CodeViewLineSelection,
	type VirtualFileMetrics,
} from "@pierre/diffs";

export type Annotation =
	/** Local review comments. */
	| { _tag: "local"; id: string }
	/** A diff comment thread on the branch's forge review. */
	| { _tag: "forge"; threadId: string }
	/** Workaround to render images w/o native library support. */
	| { _tag: "image" };

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
	file: FileAddress;
	fileId: string;
	change: TreeChange;
	treeChangeDiff: UnifiedPatch | null;
	patch: string;
	version: number;
};

export type DiffViewFile = {
	address: FileAddress;
	item: CodeViewDiffItem<Annotation>;
	change: TreeChange;
	patch: UnifiedPatch | null;
	hunks: Array<DiffViewHunk>;
};

type DiffViewHunk = {
	address: HunkAddress;
	selectedLines: CodeViewLineSelection;
	file: DiffViewFile;
};

export type DiffView = {
	addressSpace: AddressSpace<HunkAddress>;
	items: Array<CodeViewItem<Annotation>>;
	fileByItemId: Map<string, DiffViewFile>;
	fileByPath: Map<string, DiffViewFile>;
	hunkByKey: Map<string, DiffViewHunk>;
};

export const hunkAddressIdentityKey = (address: HunkAddress): string =>
	addressIdentityKey(hunkAddress(address));

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
		const file: FileAddress = { parent: fileParent, path: change.path };
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
	firstSelection: DiffLineSelection | null;
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
	const file: FileAddress = {
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
					const range = rangeFromLineGroups(fstSelection.lineGroups);
					if (!range) return { itemId, firstSelection: null };
					return {
						itemId,
						firstSelection: { file, range },
					};
				}
			}
		}
	}

	return { itemId, firstSelection: null };
};

/** Build relationships between our SDK data and Pierre's view. */
export const getDiffView = (files: Array<PreparedDiffFile>): DiffView => {
	const addressSpace: AddressSpace<HunkAddress> = {
		items: [],
		indexByKey: new Map(),
	};

	const items: Array<CodeViewItem<Annotation>> = [];

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
			...(mdiff?.type === "Patch" && isSvgFile(change.path)
				? {
						annotations: [
							{
								lineNumber: 0,
								side: change.status.type === "Deletion" ? "deletions" : "additions",
								metadata: { _tag: "image" as const },
							},
						],
					}
				: {}),
		};

		const renderItem: CodeViewDiffItem<Annotation> | CodeViewFileItem<Annotation> =
			// Construct a synthetic diff using annotations as a workaround for rendering images.
			mdiff?.type === "Binary" && isRasterImageFile(change.path)
				? {
						type: "file",
						id: fileId,
						version,
						file: { name: change.path, contents: "" },
						annotations: [{ lineNumber: 0, metadata: { _tag: "image" } }],
					}
				: item;

		items.push(renderItem);

		const diffViewFile: DiffViewFile = {
			address: file,
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

					const hunkAddress: HunkAddress = {
						parent: file,
						...selection,
						isResultOfBinaryToTextConversion: mdiff.subject.isResultOfBinaryToTextConversion,
					};
					const hunkKey = hunkAddressIdentityKey(hunkAddress);

					const len = addressSpace.items.push(hunkAddress);
					addressSpace.indexByKey.set(hunkKey, len - 1);

					const diffViewHunk: DiffViewHunk = {
						address: hunkAddress,
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
		addressSpace,
	};
};

/**
 * The address space with folded files' hunks removed — except each folded
 * file's first hunk, which stands in for the file the way a folded branch
 * keeps its branch row. j/k then stop once per folded file instead of walking
 * its hidden hunks, and z can unfold from the keyboard.
 */
export const withoutFoldedHunks = (
	addressSpace: AddressSpace<HunkAddress>,
	hunkByKey: DiffView["hunkByKey"],
	collapsedItems: Set<string>,
): AddressSpace<HunkAddress> => {
	if (collapsedItems.size === 0) return addressSpace;

	const items = addressSpace.items.filter((hunk) => {
		const key = hunkAddressIdentityKey(hunk);
		const file = hunkByKey.get(key)?.file;
		return (
			file === undefined ||
			!collapsedItems.has(file.item.id) ||
			hunkAddressIdentityKey(assert(file.hunks[0]).address) === key
		);
	});
	return { items, indexByKey: buildIndexByKey(items, hunkAddressIdentityKey) };
};
