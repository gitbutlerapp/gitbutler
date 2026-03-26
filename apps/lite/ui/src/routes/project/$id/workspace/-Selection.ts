import { Match } from "effect";
import { getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	changesDetailsItem,
	changesSummaryItem,
	itemKey,
	type Item,
	segmentItem,
	commitSummaryItem,
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
	headInfo,
	changes,
	assignments,
}: {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
}): NavigationModel => {
	const items: Array<Item> = [];
	const rootItems: Array<Item> = [];
	const rootIndexByItemIndex: Array<number> = [];
	const indexByKey = new Map<string, number>();

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
				const commitItem = commitSummaryItem({
					stackId: stack.id,
					segmentIndex,
					branchName,
					branchRef,
					commitId: commit.id,
				});
				indexByKey.set(itemKey(commitItem), items.length);
				rootIndexByItemIndex.push(rootIndex);
				items.push(commitItem);
			}
		}
	}

	return { items, rootItems, rootIndexByItemIndex, indexByKey };
};

export const getAdjacentLinearItem = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	const currentIndex = selection ? (model.indexByKey.get(itemKey(selection)) ?? -1) : -1;
	if (currentIndex === -1) return null;
	return model.items[currentIndex + offset] ?? null;
};

export const getAdjacentRootItem = (
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
