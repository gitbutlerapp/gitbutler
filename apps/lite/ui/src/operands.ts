import { Match } from "effect";
import type { HunkLineSelection } from "#ui/hunk.ts";

export type Operand =
	| { _tag: "UncommittedChanges" }
	| ({ _tag: "Branch" } & BranchOperand)
	| ({ _tag: "Commit" } & CommitOperand)
	| ({ _tag: "File" } & FileOperand)
	| ({ _tag: "Hunk" } & HunkOperand);

export type FileParent = Extract<Operand, { _tag: "UncommittedChanges" | "Branch" | "Commit" }>;

export type BranchOperand = {
	branchRef: Array<number>;
};

/**
 * The commit operand holds two forms of identity, the commit ID and the change ID, corresponding to
 * strong and weak identity respectively. Use one or the other as needed.
 */
export type CommitOperand = {
	commitId: string;
	changeId: string;
};

export type FileOperand = {
	parent: FileParent;
	path: string;
};

export type HunkOperand = HunkLineSelection & {
	parent: FileOperand;
	isResultOfBinaryToTextConversion: boolean;
};

export const uncommittedChangesOperand: Operand = {
	_tag: "UncommittedChanges",
};

export const branchOperand = ({ branchRef }: BranchOperand): Operand => ({
	_tag: "Branch",
	branchRef,
});

export const commitOperand = ({
	commitId,
	changeId,
}: CommitOperand): Extract<Operand, { _tag: "Commit" }> => ({
	_tag: "Commit",
	commitId,
	changeId,
});

export const fileOperand = ({ parent, path }: FileOperand): Extract<Operand, { _tag: "File" }> => ({
	_tag: "File",
	parent,
	path,
});

export const hunkOperand = ({
	parent,
	isResultOfBinaryToTextConversion,
	...lineSelection
}: HunkOperand): Extract<Operand, { _tag: "Hunk" }> => ({
	_tag: "Hunk",
	parent,
	isResultOfBinaryToTextConversion,
	...lineSelection,
});

export const uncommittedChangesFileParent: FileParent = {
	_tag: "UncommittedChanges",
};

export const branchFileParent = ({ branchRef }: BranchOperand): FileParent => ({
	_tag: "Branch",
	branchRef,
});

export const commitFileParent = ({ commitId, changeId }: CommitOperand): FileParent => ({
	_tag: "Commit",
	commitId,
	changeId,
});

const uncommittedChangesIdentityKey = "uncommitted_changes";

export const branchIdentityKey = (operand: BranchOperand) =>
	`branch:${operand.branchRef.join(",")}`;

export const commitIdentityKey = (operand: Pick<CommitOperand, "commitId">) =>
	`commit:${operand.commitId}`;

export const weakCommitIdentityKey = (operand: Pick<CommitOperand, "changeId">) =>
	`commit:${operand.changeId}`;

const fileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "Branch":
			return branchIdentityKey(fp);
		case "Commit":
			return commitIdentityKey(fp);
	}
};

export const weakFileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "Branch":
			return branchIdentityKey(fp);
		case "Commit":
			return weakCommitIdentityKey(fp);
	}
};

const fileIdentityKey = (operand: FileOperand) =>
	`file:${operand.path} <- ${fileParentIdentityKey(operand.parent)}`;

export const weakFileIdentityKey = (operand: FileOperand) =>
	`file:${operand.path} <- ${weakFileParentIdentityKey(operand.parent)}`;

const hunkIdentityKey = (operand: HunkOperand) =>
	`hunk:${JSON.stringify(operand.hunkHeader)}:${JSON.stringify(operand.lineGroups)}:${operand.isResultOfBinaryToTextConversion} <- ${fileIdentityKey(operand.parent)}`;

export const operandIdentityKey = (operand: Operand): string => {
	switch (operand._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "File":
			return fileIdentityKey(operand);
		case "Branch":
			return branchIdentityKey(operand);
		case "Commit":
			return commitIdentityKey(operand);
		case "Hunk":
			return hunkIdentityKey(operand);
	}
};

export const operandEquals = (a: Operand, b: Operand): boolean =>
	operandIdentityKey(a) === operandIdentityKey(b);

export const operandFileParent = (operand: Operand): FileParent | null =>
	Match.value(operand).pipe(
		Match.withReturnType<FileParent | null>(),
		Match.tags({
			File: ({ parent }) => parent,
			UncommittedChanges: () => uncommittedChangesOperand,
			Hunk: ({ parent }) => parent.parent,
		}),
		Match.orElse(() => null),
	);

/**
 * The operands if they are all files under `fileParent`, none otherwise. Checking is confined to
 * one parent at a time, so a set that isn't wholly ours belongs to another list, and its paths
 * would resolve against the wrong parent.
 */
export const filesUnder = (operands: Array<Operand>, fileParent: FileParent): Array<Operand> =>
	operands.every((operand) => operand._tag === "File" && operandEquals(operand.parent, fileParent))
		? operands
		: [];

export const operandContains = (a: Operand, b: Operand) => {
	const bFileParent = operandFileParent(b);
	return bFileParent && operandEquals(a, bFileParent);
};
