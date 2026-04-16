import {
	changesSectionFileParent,
	commitFileParent,
	type FileParent,
} from "#ui/domain/FileParent.ts";
import { type HunkHeader } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type Item } from "./Item.ts";

/** @public */
export type ChangesSectionOperationSource = {};
/** @public */
export type CommitOperationSource = { commitId: string };
/** @public */
export type FileOperationSource = { parent: FileParent; path: string };
/** @public */
export type HunkOperationSource = { parent: FileParent; path: string; hunkHeader: HunkHeader };
/** @public */
export type StackOperationSource = { stackId: string };
export type BranchOperationSource = { branchRef: Array<number> };

/**
 * The source of an operation before it has been materialized into data that can
 * be sent to the backend (`ResolvedOperationSource`).
 */
export type OperationSource =
	| { _tag: "BaseCommit" }
	| ({ _tag: "ChangesSection" } & ChangesSectionOperationSource)
	| ({ _tag: "Commit" } & CommitOperationSource)
	| ({ _tag: "File" } & FileOperationSource)
	| ({ _tag: "Hunk" } & HunkOperationSource)
	| ({ _tag: "Stack" } & StackOperationSource)
	| ({ _tag: "Branch" } & BranchOperationSource);

/** @public */
export const baseCommitOperationSource: OperationSource = {
	_tag: "BaseCommit",
};

/** @public */
export const changesSectionOperationSource = (
	_x: ChangesSectionOperationSource,
): OperationSource => ({
	_tag: "ChangesSection",
});

/** @public */
export const commitOperationSource = ({ commitId }: CommitOperationSource): OperationSource => ({
	_tag: "Commit",
	commitId,
});

/** @public */
export const fileOperationSource = ({ parent, path }: FileOperationSource): OperationSource => ({
	_tag: "File",
	parent,
	path,
});

/** @public */
export const hunkOperationSource = ({
	parent,
	path,
	hunkHeader,
}: HunkOperationSource): OperationSource => ({
	_tag: "Hunk",
	parent,
	path,
	hunkHeader,
});

/** @public */
export const stackOperationSource = ({ stackId }: StackOperationSource): OperationSource => ({
	_tag: "Stack",
	stackId,
});

/** @public */
export const branchOperationSource = ({ branchRef }: BranchOperationSource): OperationSource => ({
	_tag: "Branch",
	branchRef,
});

const operationSourceIdentityKey = (operationSource: OperationSource): string =>
	Match.value(operationSource).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => JSON.stringify(["BaseCommit"]),
			ChangesSection: () => JSON.stringify(["ChangesSection"]),
			Commit: ({ commitId }) => JSON.stringify(["Commit", commitId]),
			File: ({ parent, path }) => JSON.stringify(["File", parent, path]),
			Hunk: ({ parent, path, hunkHeader }) => JSON.stringify(["Hunk", parent, path, hunkHeader]),
			Stack: ({ stackId }) => JSON.stringify(["Stack", stackId]),
			Branch: ({ branchRef }) => JSON.stringify(["Branch", branchRef]),
		}),
	);

export const operationSourceEquals = (a: OperationSource, b: OperationSource): boolean =>
	operationSourceIdentityKey(a) === operationSourceIdentityKey(b);

export const operationSourceFromItem = (item: Item): OperationSource =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => baseCommitOperationSource,
			Change: ({ path }) =>
				fileOperationSource({
					parent: changesSectionFileParent({}),
					path,
				}),
			ChangesSection: () => changesSectionOperationSource({}),
			Commit: ({ commitId }) => commitOperationSource({ commitId }),
			CommitFile: ({ commitId, path }) =>
				fileOperationSource({
					parent: commitFileParent({ commitId }),
					path,
				}),
			Stack: ({ stackId }) => stackOperationSource({ stackId }),
			Branch: ({ branchRef }) => branchOperationSource({ branchRef }),
		}),
	);

export const operationSourceMatchesItem = (source: OperationSource, item: Item): boolean =>
	operationSourceEquals(source, operationSourceFromItem(item));
