import {
	branchFileParent,
	commitFileParent,
	hunkAddress,
	addressEquals,
	addressIdentityKey,
	type BranchAddress,
	type FileParent,
	type HunkAddress,
	type Address,
} from "#ui/addresses.ts";

/**
 * The app's named lists, each with one cursor. A cursor stores the identity
 * of the item the list rests on — never a position — and every read resolves
 * it against whatever the list currently shows. All but `diff` live in the
 * URL (see cursor-url.ts); `diff` lives in the store because its identity is
 * the exact selected line groups, which no legible string carries.
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
	diff: HunkAddress;
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
	diff: HunkAddress | null;
};

const pathKey = (path: string): string => path;

/** One identity key per list; resolution and no-op guards share it. */
export const cursorKey: { [L in CursorName]: (item: CursorItem[L]) => string } = {
	applied: addressIdentityKey,
	unapplied: addressIdentityKey,
	upstream: addressIdentityKey,
	uncommitted: pathKey,
	files: pathKey,
	diff: (address) => addressIdentityKey(hunkAddress(address)),
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
	diff: HunkAddress,
	replacedCommits: Record<string, string>,
): HunkAddress => {
	const parent = remapFileParent(diff.parent.parent, replacedCommits);
	return parent === diff.parent.parent ? diff : { ...diff, parent: { ...diff.parent, parent } };
};

export const remapDiffCursorBranch = (
	diff: HunkAddress,
	oldBranch: BranchAddress,
	newBranch: BranchAddress,
): HunkAddress => {
	const parent = diff.parent.parent;
	if (parent._tag !== "Branch" || !addressEquals(parent, branchFileParent(oldBranch))) return diff;

	return { ...diff, parent: { ...diff.parent, parent: branchFileParent(newBranch) } };
};
