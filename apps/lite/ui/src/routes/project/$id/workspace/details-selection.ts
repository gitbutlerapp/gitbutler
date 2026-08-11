import { Match } from "effect";
import type { BranchOperand, CommitOperand, Operand } from "#ui/operands.ts";

/**
 * What the details pane is showing.
 *
 * The outline tabs select from one operand vocabulary, but they are not
 * looking at the same things: a branch applied to the workspace has a position
 * — a base to diff against, a push status, a pull request — that an unapplied
 * one does not. Naming the two apart is what keeps the pane from having to
 * work out which tab it was reached from.
 */
export type DetailsSelection =
	| ({ _tag: "AppliedBranch" } & BranchOperand)
	| ({ _tag: "UnappliedBranch" } & BranchOperand)
	| ({ _tag: "Commit" } & CommitOperand)
	| { _tag: "UncommittedFile"; path: string };

export const uncommittedFileSelection = (path: string): DetailsSelection => ({
	_tag: "UncommittedFile",
	path,
});

const commitSelection = ({ commitId, changeId }: CommitOperand): DetailsSelection => ({
	_tag: "Commit",
	commitId,
	changeId,
});

/**
 * An operand from a tab showing what the workspace holds. Operands with no
 * details view of their own resolve to nothing.
 */
export const appliedSelection = (operand: Operand | null): DetailsSelection | null =>
	operand === null
		? null
		: Match.value(operand).pipe(
				Match.tags({
					Branch: ({ branchRef }): DetailsSelection => ({ _tag: "AppliedBranch", branchRef }),
					Commit: commitSelection,
				}),
				Match.orElse(() => null),
			);

/** An operand from the branches tab, which lists what the workspace does not hold. */
export const unappliedSelection = (operand: Operand | null): DetailsSelection | null =>
	operand === null
		? null
		: Match.value(operand).pipe(
				Match.tags({
					Branch: ({ branchRef }): DetailsSelection => ({ _tag: "UnappliedBranch", branchRef }),
					Commit: commitSelection,
				}),
				Match.orElse(() => null),
			);
