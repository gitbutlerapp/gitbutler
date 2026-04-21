import { Match } from "effect";
import { type FileParent } from "#ui/domain/FileParent.ts";
import { TreeChange } from "@gitbutler/but-sdk";
import { TreeChangeWithHunkHeaders } from "./ResolvedOperationSource";

export type ChangesSectionItem = {
	treeChanges: Array<TreeChange>;
};

/** @public */
export type ChangeItem = {
	treeChange: TreeChange;
};

/** @public */
export type StackItem = {
	stackId: string;
};

/** @public */
export type BranchItem = StackItem & {
	branchRef: Array<number>;
};
/** @public */
export type CommitItem = StackItem & { commitId: string };
/** @public */
export type CommitFileItem = CommitItem & {
	treeChange: TreeChange;
};

/** @public */
export type HunkOperationSource = {
	parent: FileParent;
	treeChange: TreeChangeWithHunkHeaders;
};

/**
 * A selectable item in the primary panel.
 */
export type Item =
	| ({ _tag: "ChangesSection" } & ChangesSectionItem)
	| ({ _tag: "ChangeFile" } & ChangeItem)
	| ({ _tag: "Stack" } & StackItem)
	| ({ _tag: "Branch" } & BranchItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "CommitFile" } & CommitFileItem)
	| { _tag: "BaseCommit" }
	| ({ _tag: "Hunk" } & HunkOperationSource);

/** @public */
export const changesSectionItem = ({ treeChanges }: ChangesSectionItem): Item => ({
	_tag: "ChangesSection",
	treeChanges,
});

/** @public */
export const changeFileItem = ({ treeChange }: ChangeItem): Item => ({
	_tag: "ChangeFile",
	treeChange,
});

/** @public */
export const stackItem = ({ stackId }: StackItem): Item => ({
	_tag: "Stack",
	stackId,
});

/** @public */
export const branchItem = ({ stackId, branchRef }: BranchItem): Item => ({
	_tag: "Branch",
	stackId,
	branchRef,
});

/** @public */
export const commitItem = ({ stackId, commitId }: CommitItem): Item => ({
	_tag: "Commit",
	stackId,
	commitId,
});

/** @public */
export const commitFileItem = ({ stackId, commitId, treeChange }: CommitFileItem): Item => ({
	_tag: "CommitFile",
	stackId,
	commitId,
	treeChange,
});

/** @public */
export const hunkItem = ({ parent, treeChange }: HunkOperationSource): Item => ({
	_tag: "Hunk",
	parent,
	treeChange,
});

/** @public */
export const baseCommitItem: Item = {
	_tag: "BaseCommit",
};

/**
 * Key `Item` with respect to the user interface.
 */
export const itemIdentityKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			ChangesSection: () => JSON.stringify(["ChangesSection"]),
			ChangeFile: (item) => JSON.stringify(["ChangeFile", item.treeChange]),
			Stack: (item) => JSON.stringify(["Stack", item.stackId]),
			Branch: (item) => JSON.stringify(["Branch", item.stackId, item.branchRef]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.commitId]),
			CommitFile: (item) =>
				JSON.stringify(["CommitFile", item.stackId, item.commitId, item.treeChange]),
			BaseCommit: () => JSON.stringify(["BaseCommit"]),
			Hunk: (item) => JSON.stringify(["Hunk", item.parent, item.treeChange]),
		}),
	);

/**
 * Determine `Item` equivalence with respect to the user interface. See also `itemIdentityKey`.
 */
export const itemEquals = (a: Item, b: Item): boolean => itemIdentityKey(a) === itemIdentityKey(b);
