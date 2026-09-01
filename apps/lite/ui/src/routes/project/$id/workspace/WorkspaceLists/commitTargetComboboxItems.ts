import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle } from "#ui/commit.ts";
import { addressEquals, type Address } from "#ui/addresses.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import type { CommitTargetComboboxItem } from "../CommitForm.tsx";
import { reverseValues } from "#ui/iterator.ts";

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
			? reverseValues(headInfo.stacks).flatMap(
					(stack): IteratorObject<CommitTargetComboboxItem> =>
						stack.segments
							.values()
							.map(({ refName }): CommitTargetComboboxItem | null => {
								if (!refName) return null;

								return {
									label: refName.displayName,
									address: { _tag: "Branch", branchRef: refName.fullNameBytes },
									relativeTo: {
										type: "referenceBytes",
										subject: refName.fullNameBytes,
									},
								};
							})
							.filter((x) => x != null),
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
