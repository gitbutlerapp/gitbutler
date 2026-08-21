import {
	branchFileParent,
	commitFileParent,
	addressEquals,
	addressIdentityKey,
	weakFileIdentityKey,
	type BranchAddress,
	type FileAddress,
	type FileParent,
	type Address,
} from "#ui/addresses.ts";
import type { SelectedLineRange } from "@pierre/diffs";

export type DiffLineSelection = {
	file: FileAddress;
	range: SelectedLineRange;
};

/**
 * The app's named lists, each with one cursor. The five URL-backed cursors store
 * item identity and resolve it against what their list currently shows. `diff`
 * is the exception: it stores a file identity plus Pierre's exact visual line
 * range in Redux because that range does not belong in the URL.
 *
 * `uncommitted` and `files` stay path-keyed on purpose: a bare path survives
 * root changes (select the next commit, stay on the same file). Do not
 * "upgrade" them to File addresses — the embedded parent would go stale.
 */
export type CursorItem = {
	applied: Address;
	uncommitted: string;
	unapplied: Address;
	upstream: Address;
	files: string;
	diff: DiffLineSelection;
};

export type CursorName = keyof CursorItem;

/**
 * The workspace page's cursors, snapshotted by pending operations that restore on cancel.
 * URL cursors are held in their encoded form: restoration writes the params
 * back verbatim, no resolution needed.
 */
export type WorkspaceCursorSnapshot = {
	page?: "upstream" | "branches";
	active?: "uncommitted";
	applied?: string;
	uncommitted?: string;
	files?: string;
	diff: DiffLineSelection | null;
};

const pathKey = (path: string): string => path;

/** One identity key per list; resolution and no-op guards share it. */
export const cursorKey: { [L in CursorName]: (item: CursorItem[L]) => string } = {
	applied: addressIdentityKey,
	unapplied: addressIdentityKey,
	upstream: addressIdentityKey,
	uncommitted: pathKey,
	files: pathKey,
	diff: ({ file, range }) =>
		`${weakFileIdentityKey(file)}\u0000${range.start}\u0000${range.side ?? "additions"}\u0000${range.end}\u0000${range.endSide ?? range.side ?? "additions"}`,
};

/* The diff cursor is store-held, so history rewrites remap it in the store;
   URL cursors are remapped as params (use-cursor.ts). Each remap returns its
   input unchanged (same reference) when the rewrite does not touch it. */

const remapFileParent = (parent: FileParent, replaced: Record<string, string>): FileParent => {
	if (parent._tag !== "Commit") return parent;

	const newId = replaced[parent.commitId];
	return newId === undefined
		? parent
		: commitFileParent({ commitId: newId, changeId: parent.changeId });
};

export const remapDiffCursor = (
	diff: DiffLineSelection,
	replacedCommits: Record<string, string>,
): DiffLineSelection => {
	const parent = remapFileParent(diff.file.parent, replacedCommits);
	return parent === diff.file.parent ? diff : { ...diff, file: { ...diff.file, parent } };
};

export const remapDiffCursorBranch = (
	diff: DiffLineSelection,
	oldBranch: BranchAddress,
	newBranch: BranchAddress,
): DiffLineSelection => {
	const parent = diff.file.parent;
	if (parent._tag !== "Branch" || !addressEquals(parent, branchFileParent(oldBranch))) return diff;

	return { ...diff, file: { ...diff.file, parent: branchFileParent(newBranch) } };
};
