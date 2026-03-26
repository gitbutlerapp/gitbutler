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
	ChangesItem,
	CommitItem,
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

export type SharedSelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "MoveRootDown" }
	| { _tag: "MoveRootUp" };

type ChangesSelectionAction = SharedSelectionAction;
type SegmentSelectionAction = SharedSelectionAction;
type CommitSelectionAction =
	| SharedSelectionAction
	| { _tag: "EditCommitMessage" }
	| { _tag: "ExpandCommit" };

const getSharedSelectionAction = (event: KeyboardEvent): SharedSelectionAction | null =>
	Match.value(event.key).pipe(
		Match.whenOr(
			"ArrowUp",
			"k",
			(): SharedSelectionAction => ({
				_tag: "Move",
				offset: -1,
			}),
		),
		Match.whenOr(
			"ArrowDown",
			"j",
			(): SharedSelectionAction => ({
				_tag: "Move",
				offset: 1,
			}),
		),
		Match.when("J", (): SharedSelectionAction => ({ _tag: "MoveRootDown" })),
		Match.when("K", (): SharedSelectionAction => ({ _tag: "MoveRootUp" })),
		Match.orElse((): SharedSelectionAction | null => null),
	);

export const getChangesSelectionAction = (
	selection: ChangesItem,
	event: KeyboardEvent,
): ChangesSelectionAction | null =>
	getSharedSelectionAction(event) ??
	Match.value(event.key).pipe(
		Match.when("ArrowLeft", (): ChangesSelectionAction | null =>
			selection.mode._tag === "Details" && !event.repeat ? { _tag: "MoveRootUp" } : null,
		),
		Match.orElse((): ChangesSelectionAction | null => null),
	);

export const getSegmentSelectionAction = (event: KeyboardEvent): SegmentSelectionAction | null =>
	getSharedSelectionAction(event);

export const getCommitSelectionAction = (
	selection: CommitItem,
	event: KeyboardEvent,
): CommitSelectionAction | null =>
	getSharedSelectionAction(event) ??
	Match.value(event.key).pipe(
		Match.when("Enter", (): CommitSelectionAction | null =>
			selection.mode._tag === "Summary" && !event.repeat ? { _tag: "EditCommitMessage" } : null,
		),
		Match.when("ArrowRight", (): CommitSelectionAction | null =>
			selection.mode._tag === "Summary" && !event.repeat ? { _tag: "ExpandCommit" } : null,
		),
		Match.orElse((): CommitSelectionAction | null => null),
	);
