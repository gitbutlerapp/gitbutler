import { getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { type ShortcutBinding } from "#ui/shortcuts.ts";
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

const createSharedSelectionBindings = <Context>(): Array<
	ShortcutBinding<SharedSelectionAction, Context>
> => [
	{
		id: "move-up",
		description: "up",
		keys: ["ArrowUp", "k"],
		action: { _tag: "Move", offset: -1 },
	},
	{
		id: "move-down",
		description: "down",
		keys: ["ArrowDown", "j"],
		action: { _tag: "Move", offset: 1 },
	},
	{
		id: "move-root-down",
		description: "next section",
		keys: ["J"],
		action: { _tag: "MoveRootDown" },
	},
	{
		id: "move-root-up",
		description: "previous section",
		keys: ["K"],
		action: { _tag: "MoveRootUp" },
	},
];

export const changesSelectionBindings: Array<ShortcutBinding<ChangesSelectionAction, ChangesItem>> =
	[
		...createSharedSelectionBindings<ChangesItem>(),
		{
			id: "changes-previous-section",
			description: "previous section",
			keys: ["ArrowLeft"],
			action: { _tag: "MoveRootUp" },
			repeat: false,
			when: (selection) => selection.mode._tag === "Details",
		},
	];

export const segmentSelectionBindings: Array<ShortcutBinding<SegmentSelectionAction>> =
	createSharedSelectionBindings<void>();

export const commitSelectionBindings: Array<ShortcutBinding<CommitSelectionAction, CommitItem>> = [
	...createSharedSelectionBindings<CommitItem>(),
	{
		id: "commit-edit-message",
		description: "edit message",
		keys: ["Enter"],
		action: { _tag: "EditCommitMessage" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Summary",
	},
	{
		id: "commit-expand",
		description: "details",
		keys: ["ArrowRight", "l"],
		action: { _tag: "ExpandCommit" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Summary",
	},
];
