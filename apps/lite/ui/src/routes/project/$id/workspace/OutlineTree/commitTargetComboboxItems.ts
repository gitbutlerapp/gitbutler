import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle } from "#ui/commit.ts";
import { operandEquals, type Operand } from "#ui/operands.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import type { CommitTargetComboboxItem } from "../CommitForm.tsx";

export const buildCommitTargetComboboxItems = ({
	headInfo,
	headInfoIndex,
	outlineSelection,
}: {
	headInfo: RefInfo | undefined;
	headInfoIndex: HeadInfoIndex | undefined;
	outlineSelection: Operand | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		outlineSelection?._tag === "Commit"
			? headInfoIndex?.commitContextById(outlineSelection.commitId)?.commit
			: null;

	return [
		...(commitTarget
			? ([
					{
						label: commitTitle(commitTarget.message) ?? "(no message)",
						operand: { _tag: "Commit", commitId: commitTarget.id, changeId: commitTarget.changeId },
						relativeTo: { type: "commit", subject: commitTarget.id },
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
									operand: { _tag: "Branch", branchRef: refName.fullNameBytes },
									relativeTo: {
										type: "referenceBytes",
										subject: refName.fullNameBytes,
									},
								},
							];
						}),
				)
			: []),
	];
};

export const selectCommitTargetComboboxItem = ({
	items,
	outlineSelection,
}: {
	items: Array<CommitTargetComboboxItem>;
	outlineSelection: Operand | null;
}): CommitTargetComboboxItem | null =>
	(outlineSelection && items.find((item) => operandEquals(item.operand, outlineSelection))) ?? null;
