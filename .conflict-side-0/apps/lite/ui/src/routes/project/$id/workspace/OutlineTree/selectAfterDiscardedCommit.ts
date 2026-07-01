import {
	commitOperand,
	operandIdentityKey,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import { type NavigationIndex } from "#ui/workspace/navigation-index.ts";

const isCommitDiscardBoundary = (operand: Operand): boolean =>
	operand._tag === "Branch" || operand._tag === "UncommittedChanges";

export const selectAfterDiscardedCommit = ({
	navigationIndex,
	commit,
}: {
	navigationIndex: NavigationIndex<Operand>;
	commit: CommitOperand;
}): Operand | null => {
	const commitIndex = navigationIndex.indexByKey.get(operandIdentityKey(commitOperand(commit)));
	if (commitIndex === undefined) return null;

	for (const item of navigationIndex.items.slice(commitIndex + 1)) {
		if (isCommitDiscardBoundary(item)) break;
		if (item._tag === "Commit") return item;
	}

	for (const item of navigationIndex.items.slice(0, commitIndex).reverse()) {
		if (isCommitDiscardBoundary(item)) break;
		if (item._tag === "Commit") return item;
	}

	for (const item of navigationIndex.items.slice(0, commitIndex + 1).reverse()) {
		if (item._tag === "Branch") return item;
		if (isCommitDiscardBoundary(item)) break;
	}

	return null;
};
