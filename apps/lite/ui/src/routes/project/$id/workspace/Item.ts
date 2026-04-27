import { Match } from "effect";
import { changesFileParent, type FileParent } from "#ui/domain/FileParent.ts";
import { type HunkHeader } from "@gitbutler/but-sdk";

/** @public */
export type StackItem = {
	stackId: string;
};

/** @public */
export type BranchItem = StackItem & {
	branchRef: Array<number>;
};

/** @public */
export type CommitItem = StackItem & {
	commitId: string;
};

/** @public */
export type FileItem = {
	parent: FileParent;
	path: string;
};

/** @public */
export type HunkItem = FileItem & {
	hunkHeader: HunkHeader;
	isResultOfBinaryToTextConversion: boolean;
};

export type Item =
	| { _tag: "ChangesSection" }
	| ({ _tag: "Stack" } & StackItem)
	| ({ _tag: "Branch" } & BranchItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "File" } & FileItem)
	| ({ _tag: "Hunk" } & HunkItem)
	| { _tag: "BaseCommit" };

/** @public */
export const changesSectionItem: Item = {
	_tag: "ChangesSection",
};

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
export const fileItem = ({ parent, path }: FileItem): Item => ({
	_tag: "File",
	parent,
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
			File: (item) => JSON.stringify(["File", item.parent, item.path]),
			Stack: (item) => JSON.stringify(["Stack", item.stackId]),
			Branch: (item) => JSON.stringify(["Branch", item.stackId, item.branchRef]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.commitId]),
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
			File: ({ parent }) => parent,
			ChangesSection: () => changesFileParent,
			Hunk: ({ parent }) => parent,
		}),
		Match.orElse(() => null),
	);
