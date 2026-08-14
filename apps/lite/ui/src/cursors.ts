import {
	branchFileParent,
	commitFileParent,
	hunkOperand,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type FileParent,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";

/**
 * The app's named lists, each with one cursor. A cursor stores the identity
 * of the item the list rests on — never a position — and every read resolves
 * it against whatever the list currently shows. All but `diff` live in the
 * URL (see cursor-url.ts); `diff` lives in the store because its identity is
 * the exact selected line groups, which no legible string carries.
 *
 * `uncommitted` and `files` stay path-keyed on purpose: a bare path survives
 * root changes (select the next commit, stay on the same file). Do not
 * "upgrade" them to File operands — the embedded parent would go stale.
 */
export type ListItem = {
	stacks: Operand;
	uncommitted: string;
	branches: Operand;
	upstream: Operand;
	files: string;
	diff: HunkOperand;
};

export type ListName = keyof ListItem;

/**
 * The workspace page's cursors, snapshotted by modes that restore on cancel.
 * URL cursors are held in their encoded form: restoration writes the params
 * back verbatim, no resolution needed.
 */
export type WorkspaceCursorSnapshot = {
	stacks?: string;
	uncommitted?: string;
	files?: string;
	diff: HunkOperand | null;
};

const pathKey = (path: string): string => path;

/** One identity key per list; resolution and no-op guards share it. */
export const cursorKey: { [L in ListName]: (item: ListItem[L]) => string } = {
	stacks: operandIdentityKey,
	branches: operandIdentityKey,
	upstream: operandIdentityKey,
	uncommitted: pathKey,
	files: pathKey,
	diff: (operand) => operandIdentityKey(hunkOperand(operand)),
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
	diff: HunkOperand,
	replacedCommits: Record<string, string>,
): HunkOperand => {
	const parent = remapFileParent(diff.parent.parent, replacedCommits);
	return parent === diff.parent.parent ? diff : { ...diff, parent: { ...diff.parent, parent } };
};

export const remapDiffCursorBranch = (
	diff: HunkOperand,
	oldBranch: BranchOperand,
	newBranch: BranchOperand,
): HunkOperand => {
	const parent = diff.parent.parent;
	if (parent._tag !== "Branch" || !operandEquals(parent, branchFileParent(oldBranch))) return diff;

	return { ...diff, parent: { ...diff.parent, parent: branchFileParent(newBranch) } };
};
