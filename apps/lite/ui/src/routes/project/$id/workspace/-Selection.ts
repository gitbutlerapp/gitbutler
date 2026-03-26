import { Match } from "effect";
import { getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	changesDetailsItem,
	changesSummaryItem,
	getParentRootItem,
	itemKey,
	type Item,
	CommitItem,
	segmentItem,
	commitDetailsItem,
	commitSummaryItem,
	commitEditingMessageItem,
} from "./-Item.ts";

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

type NavigationModel = {
	items: Array<Item>;
	rootItems: Array<Item>;
	rootIndexByItemIndex: Array<number>;
	indexByKey: Map<string, number>;
};

export const buildNavigationModel = ({
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
}): NavigationModel => {
	const items: Array<Item> = [];
	const rootItems: Array<Item> = [];
	const rootIndexByItemIndex: Array<number> = [];
	const indexByKey = new Map<string, number>();
	const commitDetails =
		selection?._tag === "Commit" && selection.mode._tag === "Details" ? selection : null;

	const addChangesItems = (stackId: string | null) => {
		const rootItem = changesSummaryItem(stackId);
		const rootIndex = rootItems.length;
		rootItems.push(rootItem);
		indexByKey.set(itemKey(rootItem), items.length);
		rootIndexByItemIndex.push(rootIndex);
		items.push(rootItem);

		for (const change of changes) {
			if (!hasAssignmentsForPath({ assignments, stackId, path: change.path })) continue;
			const item = changesDetailsItem(stackId, change.path);
			indexByKey.set(itemKey(item), items.length);
			rootIndexByItemIndex.push(rootIndex);
			items.push(item);
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
			const rootIndex = rootItems.length;
			rootItems.push(rootItem);
			indexByKey.set(itemKey(rootItem), items.length);
			rootIndexByItemIndex.push(rootIndex);
			items.push(rootItem);

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
				indexByKey.set(itemKey(commitItem), items.length);
				rootIndexByItemIndex.push(rootIndex);
				items.push(commitItem);

				if (!isCommitDetails) continue;

				for (const path of commitDetailsPaths) {
					const item = commitDetailsItem(
						{
							stackId: stack.id,
							segmentIndex,
							branchName,
							branchRef,
							commitId: commit.id,
						},
						path,
					);
					indexByKey.set(itemKey(item), items.length);
					rootIndexByItemIndex.push(rootIndex);
					items.push(item);
				}
			}
		}
	}

	return { items, rootItems, rootIndexByItemIndex, indexByKey };
};

const getAdjacentLinearItem = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	const currentIndex = selection ? (model.indexByKey.get(itemKey(selection)) ?? -1) : -1;
	if (currentIndex === -1) return null;
	return model.items[currentIndex + offset] ?? null;
};

const getAdjacentRootItem = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = model.indexByKey.get(itemKey(selection));
	if (currentIndex === undefined) return null;
	const currentRootIndex = model.rootIndexByItemIndex[currentIndex] ?? -1;
	if (currentRootIndex === -1) return null;
	return model.rootItems[currentRootIndex + offset] ?? null;
};

type SelectionAction =
	| { _tag: "Edit" }
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "MoveRootDown" }
	| { _tag: "MoveRootUp" }
	| { _tag: "Collapse" }
	| { _tag: "Expand" };

export const getSelectionAction = (event: KeyboardEvent): SelectionAction | null =>
	Match.value(event.key).pipe(
		Match.when("Enter", (): SelectionAction | null => (!event.repeat ? { _tag: "Edit" } : null)),
		Match.whenOr("ArrowUp", "k", (): SelectionAction | null => ({
			_tag: "Move",
			offset: -1,
		})),
		Match.whenOr("ArrowDown", "j", (): SelectionAction | null => ({
			_tag: "Move",
			offset: 1,
		})),
		Match.when("J", (): SelectionAction | null => ({ _tag: "MoveRootDown" })),
		Match.when("K", (): SelectionAction | null => ({ _tag: "MoveRootUp" })),
		Match.when("ArrowLeft", (): SelectionAction | null =>
			!event.repeat ? { _tag: "Collapse" } : null,
		),
		Match.when("ArrowRight", (): SelectionAction | null =>
			!event.repeat ? { _tag: "Expand" } : null,
		),
		Match.orElse((): SelectionAction | null => null),
	);

export const performSelectionAction = async ({
	action,
	model,
	selection,
	expandCommit,
}: {
	action: SelectionAction;
	model: NavigationModel;
	selection: Item;
	expandCommit: (selection: CommitItem) => Promise<Item | null>;
}): Promise<Item | null> =>
	Match.value(action).pipe(
		Match.tag("Edit", () =>
			selection._tag === "Commit" && selection.mode._tag === "Summary"
				? commitEditingMessageItem(selection)
				: null,
		),
		Match.tag("Move", ({ offset }) => getAdjacentLinearItem(model, selection, offset)),
		Match.tag("MoveRootDown", () => getAdjacentRootItem(model, selection, 1)),
		Match.tag(
			"MoveRootUp",
			() => getParentRootItem(selection) ?? getAdjacentRootItem(model, selection, -1),
		),
		Match.tag("Collapse", () =>
			selection._tag === "Commit" && selection.mode._tag === "Details"
				? commitSummaryItem(selection)
				: null,
		),
		Match.tag("Expand", () => {
			if (selection._tag !== "Commit" || selection.mode._tag !== "Summary") return null;
			return expandCommit(selection);
		}),
		Match.exhaustive,
	);
