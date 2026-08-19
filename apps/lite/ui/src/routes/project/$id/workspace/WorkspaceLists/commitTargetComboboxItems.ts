import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle } from "#ui/commit.ts";
import { addressEquals, type Address } from "#ui/addresses.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import type { CommitTargetComboboxItem } from "../CommitForm.tsx";

export const buildCommitTargetComboboxItems = ({
	headInfo,
	headInfoIndex,
	appliedSelection,
}: {
	headInfo: RefInfo | undefined;
	headInfoIndex: HeadInfoIndex | undefined;
	appliedSelection: Address | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		appliedSelection?._tag === "Commit"
			? headInfoIndex?.commitContextByCommitId(appliedSelection.commitId)?.commit
			: null;

	return [
		...(commitTarget
			? ([
					{
						label: commitTitle(commitTarget.message) ?? "(no message)",
						address: { _tag: "Commit", commitId: commitTarget.id, changeId: commitTarget.changeId },
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
									address: { _tag: "Branch", branchRef: refName.fullNameBytes },
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
	appliedSelection,
}: {
	items: Array<CommitTargetComboboxItem>;
	appliedSelection: Address | null;
}): CommitTargetComboboxItem | null =>
	(appliedSelection && items.find((item) => addressEquals(item.address, appliedSelection))) ?? null;
