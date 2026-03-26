import { getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	changesDetailsItem,
	changesSummaryItem,
	commitDetailsItem,
	commitEditingMessageItem,
	commitSummaryItem,
	getParentItem,
	itemsEqual,
	type Item,
	segmentItem,
} from "./-Item.ts";

export const toggleChangesItem = (selection: Item | null, stackId: string | null): Item | null =>
	selectItemOrParent(selection, changesSummaryItem(stackId));

export const toggleSegmentItem = (
	selection: Item | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
): Item | null =>
	selectItemOrParent(selection, segmentItem({ stackId, segmentIndex, branchName, branchRef }));

export const toggleChangesFileItem = (
	selection: Item | null,
	stackId: string | null,
	path: string,
): Item | null => selectItemOrParent(selection, changesDetailsItem(stackId, path));

export const toggleCommitItem = (
	selection: Item | null,
	stackId: string,
	segmentIndex: number,
	commitId: string,
	branchName: string | null,
	branchRef: string | null,
): Item | null =>
	selectItemOrParent(
		selection,
		commitSummaryItem({ stackId, segmentIndex, branchName, branchRef, commitId }),
	);

export const toggleCommitItemEditingMessage = (
	selection: Item | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
	commitId: string,
): Item | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag === "EditingMessage"
		? {
				...selection,
				mode: { _tag: "Summary" },
			}
		: commitEditingMessageItem({ stackId, segmentIndex, branchName, branchRef, commitId });

export const toggleCommitFileItem = (
	selection: Item | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
	commitId: string,
	path: string,
): Item | null =>
	selectItemOrParent(
		selection,
		commitDetailsItem({ stackId, segmentIndex, branchName, branchRef, commitId }, path),
	);

const selectItemOrParent = (selection: Item | null, targetItem: Item): Item | null =>
	itemsEqual(selection, targetItem) ? getParentItem(targetItem) : targetItem;

const hasAssignmentsForPath = ({
	assignments,
	stackId,
	path,
}: {
	assignments: Array<HunkAssignment>;
	stackId: string | null;
	path: string;
}): boolean =>
	assignments.some(
		(assignment) => (assignment.stackId ?? null) === stackId && assignment.path === path,
	);

export type ItemWithRoot = {
	item: Item;
	rootItem: Item;
};

export const getOrderedItems = ({
	selection,
	headInfo,
	changes,
	assignments,
	commitDetailsPaths,
}: {
	selection: Item | null;
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	commitDetailsPaths: Array<string>;
}): Array<ItemWithRoot> => {
	const items: Array<ItemWithRoot> = [];
	const commitDetails =
		selection?._tag === "Commit" && selection.mode._tag === "Details" ? selection : null;

	const addChangesItems = (stackId: string | null) => {
		const rootItem = changesSummaryItem(stackId);
		items.push({ item: rootItem, rootItem });

		for (const change of changes) {
			if (!hasAssignmentsForPath({ assignments, stackId, path: change.path })) continue;
			items.push({ item: changesDetailsItem(stackId, change.path), rootItem });
		}
	};

	addChangesItems(null);

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;
		addChangesItems(stack.id);

		for (const [segmentIndex, segment] of stack.segments.entries()) {
			const branchName = segment.refName?.displayName ?? null;
			const branchRef = segment.refName ? getSegmentBranchRef(segment.refName) : null;
			const rootItem = segmentItem({
				stackId: stack.id,
				segmentIndex,
				branchName,
				branchRef,
			});
			items.push({ item: rootItem, rootItem });

			for (const commit of segment.commits) {
				const isCommitDetails =
					commitDetails !== null &&
					commitDetails.stackId === stack.id &&
					commitDetails.segmentIndex === segmentIndex &&
					commitDetails.commitId === commit.id;
				const commitItem = isCommitDetails
					? commitDetailsItem({
							stackId: stack.id,
							segmentIndex,
							branchName,
							branchRef,
							commitId: commit.id,
						})
					: commitSummaryItem({
							stackId: stack.id,
							segmentIndex,
							branchName,
							branchRef,
							commitId: commit.id,
						});
				items.push({ item: commitItem, rootItem });

				if (!isCommitDetails) continue;

				for (const path of commitDetailsPaths)
					items.push({
						item: commitDetailsItem(
							{
								stackId: stack.id,
								segmentIndex,
								branchName,
								branchRef,
								commitId: commit.id,
							},
							path,
						),
						rootItem,
					});
			}
		}
	}

	return items;
};

export const getAdjacentLinearItem = (
	items: Array<ItemWithRoot>,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	const currentIndex = selection ? items.findIndex(({ item }) => itemsEqual(item, selection)) : -1;
	if (currentIndex === -1) return null;
	return items[currentIndex + offset]?.item ?? null;
};

export const getAdjacentRootItem = (
	items: Array<ItemWithRoot>,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentItem = items.find(({ item }) => itemsEqual(item, selection));
	if (!currentItem) return null;
	const currentRootItem = currentItem.rootItem;
	const rootItems: Array<Item> = [];

	for (const { rootItem } of items) {
		const previousRootItem = rootItems[rootItems.length - 1];
		if (previousRootItem && itemsEqual(previousRootItem, rootItem)) continue;
		rootItems.push(rootItem);
	}

	const currentRootIndex = rootItems.findIndex((item) => itemsEqual(item, currentRootItem));
	if (currentRootIndex === -1) return null;
	return rootItems[currentRootIndex + offset] ?? null;
};
