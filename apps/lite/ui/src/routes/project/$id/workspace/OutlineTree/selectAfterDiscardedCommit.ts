import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	branchOperand,
	commitOperand,
	operandIdentityKey,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";

export const selectAfterDiscardedCommit = ({
	navigationIndex,
	commit,
	headInfoIndex,
}: {
	navigationIndex: NavigationIndex<Operand>;
	commit: CommitOperand;
	headInfoIndex: HeadInfoIndex | undefined;
}): Operand | null => {
	const commitIndex = navigationIndex.indexByKey.get(operandIdentityKey(commitOperand(commit)));
	if (commitIndex === undefined) return null;

	const nextCommit = navigationIndex.items[commitIndex + 1];
	if (nextCommit?._tag === "Commit") return nextCommit;

	const prevCommit = navigationIndex.items[commitIndex - 1];
	if (prevCommit?._tag === "Commit") return prevCommit;

	const commitCtx = headInfoIndex?.commitContextByCommitId(commit.commitId);
	if (!commitCtx?.segment.refName) return null;

	const branchIdx = navigationIndex.indexByKey.get(
		operandIdentityKey(
			branchOperand({
				branchRef: commitCtx.segment.refName.fullNameBytes,
			}),
		),
	);
	if (branchIdx === undefined) return null;

	return navigationIndex.items[branchIdx] ?? null;
};
