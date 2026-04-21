import { Match } from "effect";
import { type FileParent } from "#ui/domain/FileParent.ts";
import { type HunkHeader } from "@gitbutler/but-sdk";

/** @public */
export type ChangeItem = { path: string };

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
export type CommitFileItem = CommitItem & { path: string };

/** @public */
export type HunkItem = { parent: FileParent; path: string; hunkHeader: HunkHeader };

/**
 * A selectable item in the primary panel.
 */
export type Item =
	| { _tag: "ChangesSection" }
	| ({ _tag: "ChangeFile" } & ChangeItem)
	| ({ _tag: "Stack" } & StackItem)
	| ({ _tag: "Branch" } & BranchItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "CommitFile" } & CommitFileItem)
	| { _tag: "BaseCommit" }
	| ({ _tag: "Hunk" } & HunkItem);

/** @public */
export const changesSectionItem: Item = {
	_tag: "ChangesSection",
};

/** @public */
export const changeFileItem = ({ path }: ChangeItem): Item => ({
	_tag: "ChangeFile",
	path,
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
export const commitFileItem = ({ stackId, commitId, path }: CommitFileItem): Item => ({
	_tag: "CommitFile",
	stackId,
	commitId,
	path,
});

/** @public */
export const hunkItem = ({ parent, path, hunkHeader }: HunkItem): Item => ({
	_tag: "Hunk",
	parent,
	path,
	hunkHeader,
});

/** @public */
export const baseCommitItem: Item = {
	_tag: "BaseCommit",
};

export const itemIdentityKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			ChangesSection: () => JSON.stringify(["ChangesSection"]),
			ChangeFile: (item) => JSON.stringify(["ChangeFile", item.path]),
			Stack: (item) => JSON.stringify(["Stack", item.stackId]),
			Branch: (item) => JSON.stringify(["Branch", item.stackId, item.branchRef]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.commitId]),
			CommitFile: (item) => JSON.stringify(["CommitFile", item.stackId, item.commitId, item.path]),
			BaseCommit: () => JSON.stringify(["BaseCommit"]),
			Hunk: (item) => JSON.stringify(["Hunk", item.parent, item.path, item.hunkHeader]),
		}),
	);

export const itemEquals = (a: Item, b: Item): boolean => itemIdentityKey(a) === itemIdentityKey(b);
