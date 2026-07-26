import { Match } from "effect";
import type { HunkLineSelection } from "#ui/hunk.ts";

export type Operand =
	| { _tag: "UncommittedChanges" }
	| ({ _tag: "Stack" } & StackOperand)
	| ({ _tag: "Branch" } & BranchOperand)
	| ({ _tag: "Commit" } & CommitOperand)
	| ({ _tag: "File" } & FileOperand)
	| ({ _tag: "Hunk" } & HunkOperand);

export type FileParent = Extract<Operand, { _tag: "UncommittedChanges" | "Branch" | "Commit" }>;

export type StackOperand = {
	stackId: string;
};

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

export const stackOperand = ({ stackId }: StackOperand): Operand => ({
	_tag: "Stack",
	stackId,
});

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
}: HunkOperand): Operand => ({
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

export const commitIdentityKey = (operand: Pick<CommitOperand, "commitId">) =>
	JSON.stringify(["Commit", operand.commitId]);

export const operandIdentityKey = (operand: Operand): string =>
	Match.value(operand).pipe(
		Match.tagsExhaustive({
			UncommittedChanges: () => JSON.stringify(["UncommittedChanges"]),
			File: (x) => JSON.stringify(["File", x.parent, x.path]),
			Stack: (x) => JSON.stringify(["Stack", x.stackId]),
			Branch: (x) => JSON.stringify(["Branch", x.branchRef]),
			Commit: (x) => commitIdentityKey(x),
			Hunk: (x) =>
				JSON.stringify([
					"Hunk",
					x.parent,
					x.hunkHeader,
					x.lineGroups,
					x.isResultOfBinaryToTextConversion,
				]),
		}),
	);

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

export const operandContains = (a: Operand, b: Operand) => {
	const bFileParent = operandFileParent(b);
	return bFileParent && operandEquals(a, bFileParent);
};
