import { Match } from "effect";
import {
	branchFileParent,
	changeFileParent,
	commitFileParent,
	type FileParent,
} from "#ui/domain/FileParent.ts";
import { type HunkHeader } from "@gitbutler/but-sdk";

/** @public */
export type ChangesFileItem = { path: string };

/** @public */
export type StackItem = {
	stackId: string;
};

/** @public */
export type BranchItem = StackItem & {
	branchRef: Array<number>;
};
/** @public */
export type BranchFileItem = BranchItem & { path: string };
/** @public */
export type CommitItem = StackItem & { commitId: string };
/** @public */
export type CommitFileItem = CommitItem & { path: string };

/** @public */
export type HunkItem = {
	parent: FileParent;
	path: string;
	hunkHeader: HunkHeader;
	isResultOfBinaryToTextConversion: boolean;
};

export type Item =
	| { _tag: "ChangesSection" }
	| ({ _tag: "ChangesFile" } & ChangesFileItem)
	| ({ _tag: "Stack" } & StackItem)
	| ({ _tag: "Branch" } & BranchItem)
	| ({ _tag: "BranchFile" } & BranchFileItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "CommitFile" } & CommitFileItem)
	| { _tag: "BaseCommit" }
	| ({ _tag: "Hunk" } & HunkItem);

/** @public */
export const changesSectionItem: Item = {
	_tag: "ChangesSection",
};

/** @public */
export const changesFileItem = ({ path }: ChangesFileItem): Item => ({
	_tag: "ChangesFile",
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
export const branchFileItem = ({ stackId, branchRef, path }: BranchFileItem): Item => ({
	_tag: "BranchFile",
	stackId,
	branchRef,
	path,
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
export const hunkItem = ({
	parent,
	path,
	hunkHeader,
	isResultOfBinaryToTextConversion,
}: HunkItem): Item => ({
	_tag: "Hunk",
	parent,
	path,
	hunkHeader,
	isResultOfBinaryToTextConversion,
});

/** @public */
export const baseCommitItem: Item = {
	_tag: "BaseCommit",
};

export const itemIdentityKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			ChangesSection: () => JSON.stringify(["ChangesSection"]),
			ChangesFile: (item) => JSON.stringify(["ChangesFile", item.path]),
			Stack: (item) => JSON.stringify(["Stack", item.stackId]),
			Branch: (item) => JSON.stringify(["Branch", item.stackId, item.branchRef]),
			BranchFile: (item) => JSON.stringify(["BranchFile", item.stackId, item.branchRef, item.path]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.commitId]),
			CommitFile: (item) => JSON.stringify(["CommitFile", item.stackId, item.commitId, item.path]),
			BaseCommit: () => JSON.stringify(["BaseCommit"]),
			Hunk: (item) =>
				JSON.stringify([
					"Hunk",
					item.parent,
					item.path,
					item.hunkHeader,
					item.isResultOfBinaryToTextConversion,
				]),
		}),
	);

export const itemEquals = (a: Item, b: Item): boolean => itemIdentityKey(a) === itemIdentityKey(b);

export const itemFileParent = (item: Item): FileParent | null =>
	Match.value(item).pipe(
		Match.withReturnType<FileParent | null>(),
		Match.tags({
			ChangesFile: () => changeFileParent,
			ChangesSection: () => changeFileParent,
			CommitFile: ({ stackId, commitId }) => commitFileParent({ stackId, commitId }),
			BranchFile: ({ stackId, branchRef }) => branchFileParent({ stackId, branchRef }),
			Hunk: ({ parent }) => parent,
		}),
		Match.orElse(() => null),
	);
