import { Match } from "effect";
import type { HunkLineSelection } from "#ui/hunk.ts";

/** @public */
export type BranchFileParent = { stackId: string; branchRef: Array<number> };
/** @public */
export type CommitFileParent = { stackId: string; commitId: string };

export type FileParent =
	| { _tag: "Changes" }
	| ({ _tag: "Branch" } & BranchFileParent)
	| ({ _tag: "Commit" } & CommitFileParent);

/** @public */
export const changesFileParent: FileParent = {
	_tag: "Changes",
};

/** @public */
export const branchFileParent = ({ stackId, branchRef }: BranchFileParent): FileParent => ({
	_tag: "Branch",
	stackId,
	branchRef,
});

/** @public */
export const commitFileParent = ({ stackId, commitId }: CommitFileParent): FileParent => ({
	_tag: "Commit",
	stackId,
	commitId,
});

/** @public */
export type StackOperand = {
	stackId: string;
};

/** @public */
export type BranchOperand = StackOperand & {
	branchRef: Array<number>;
};

/** @public */
export type CommitOperand = StackOperand & {
	commitId: string;
};

/** @public */
export type FileOperand = {
	parent: FileParent;
	path: string;
};

/** @public */
export type HunkOperand = HunkLineSelection & {
	parent: FileOperand;
	isResultOfBinaryToTextConversion: boolean;
};

export type Operand =
	| { _tag: "ChangesSection" }
	| ({ _tag: "Stack" } & StackOperand)
	| ({ _tag: "Branch" } & BranchOperand)
	| ({ _tag: "Commit" } & CommitOperand)
	| ({ _tag: "File" } & FileOperand)
	| ({ _tag: "Hunk" } & HunkOperand);

/** @public */
export const changesSectionOperand: Operand = {
	_tag: "ChangesSection",
};

/** @public */
export const stackOperand = ({ stackId }: StackOperand): Operand => ({
	_tag: "Stack",
	stackId,
});

/** @public */
export const branchOperand = ({ stackId, branchRef }: BranchOperand): Operand => ({
	_tag: "Branch",
	stackId,
	branchRef,
});

/** @public */
export const commitOperand = ({ stackId, commitId }: CommitOperand): Operand => ({
	_tag: "Commit",
	stackId,
	commitId,
});

/** @public */
export const fileOperand = ({ parent, path }: FileOperand): Operand => ({
	_tag: "File",
	parent,
	path,
});

/** @public */
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

export const operandIdentityKey = (operand: Operand): string =>
	Match.value(operand).pipe(
		Match.tagsExhaustive({
			ChangesSection: () => JSON.stringify(["ChangesSection"]),
			File: (x) => JSON.stringify(["File", x.parent, x.path]),
			Stack: (x) => JSON.stringify(["Stack", x.stackId]),
			Branch: (x) => JSON.stringify(["Branch", x.stackId, x.branchRef]),
			Commit: (x) => JSON.stringify(["Commit", x.stackId, x.commitId]),
			Hunk: (x) =>
				JSON.stringify([
					"Hunk",
					x.parent,
					x.hunkHeader,
					x.lineGroups,
					x.range,
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
			ChangesSection: () => changesFileParent,
			Hunk: ({ parent }) => parent.parent,
		}),
		Match.orElse(() => null),
	);

const fileParentToOperand = (fileParent: FileParent): Operand =>
	Match.value(fileParent).pipe(
		Match.tagsExhaustive({
			Changes: () => changesSectionOperand,
			Branch: ({ stackId, branchRef }) => branchOperand({ stackId, branchRef }),
			Commit: ({ stackId, commitId }) => commitOperand({ stackId, commitId }),
		}),
	);

export const operandContains = (a: Operand, b: Operand) => {
	if (operandEquals(a, b)) return true;

	const bFileParent = operandFileParent(b);
	if (bFileParent && operandEquals(a, fileParentToOperand(bFileParent))) return true;

	return false;
};
