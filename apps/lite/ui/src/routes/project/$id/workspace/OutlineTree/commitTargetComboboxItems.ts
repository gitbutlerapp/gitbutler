import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle } from "#ui/commit.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import type { CommitTargetComboboxItem } from "../CommitForm.tsx";
import { branchOperand, operandEquals, type Operand } from "#ui/operands.ts";

export const buildCommitTargetComboboxItems = ({
	headInfo,
	headInfoIndex,
	commitTargetState,
}: {
	headInfo: RefInfo | undefined;
	headInfoIndex: HeadInfoIndex | undefined;
	commitTargetState: Operand | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		commitTargetState?._tag === "Commit"
			? headInfoIndex?.commitContextById(commitTargetState.changeId)?.commit
			: null;

	return [
		...(commitTargetState && commitTarget
			? ([
					{
						label: `Commit: ${commitTitle(commitTarget.message) ?? "(no message)"}`,
						operand: commitTargetState,
					},
				] satisfies Array<CommitTargetComboboxItem>)
			: []),
		...(headInfo
			? headInfo.stacks.toReversed().flatMap(
					(stack): Array<CommitTargetComboboxItem> =>
						stack.segments.flatMap((segment): Array<CommitTargetComboboxItem> => {
							const refName = segment.refName;
							if (!refName) return [];

							return [
								{
									label: refName.displayName,
									operand: branchOperand({ branchRef: refName.fullNameBytes }),
								},
							];
						}),
				)
			: []),
	];
};

export const selectCommitTargetComboboxItem = ({
	items,
	commitTargetState,
}: {
	items: Array<CommitTargetComboboxItem>;
	commitTargetState: Operand | null;
}): CommitTargetComboboxItem | null =>
	(commitTargetState && items.find((item) => operandEquals(item.operand, commitTargetState))) ??
	items[0] ??
	null;
