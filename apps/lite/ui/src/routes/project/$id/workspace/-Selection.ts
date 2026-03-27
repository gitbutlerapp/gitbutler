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
	sections: Array<Item>;
	sectionIndexByItemIndex: Array<number>;
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
	const sections: Array<Item> = [];
	const sectionIndexByItemIndex: Array<number> = [];
	const indexByKey = new Map<string, number>();

	const addChangesItems = (stackId: string | null) => {
		const section = changesSummaryItem(stackId);
		const sectionIndex = sections.length;
		sections.push(section);
		indexByKey.set(itemKey(section), items.length);
		sectionIndexByItemIndex.push(sectionIndex);
		items.push(section);

		for (const change of changes) {
			if (!hasAssignmentsForPath({ assignments, stackId, path: change.path })) continue;
			const item = changesDetailsItem(stackId, change.path);
			indexByKey.set(itemKey(item), items.length);
			sectionIndexByItemIndex.push(sectionIndex);
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
			const section = segmentItem({
				stackId: stack.id,
				segmentIndex,
				branchName,
				branchRef,
			});
			const sectionIndex = sections.length;
			sections.push(section);
			indexByKey.set(itemKey(section), items.length);
			sectionIndexByItemIndex.push(sectionIndex);
			items.push(section);

			for (const commit of segment.commits) {
				const commitItem = commitSummaryItem({
					stackId: stack.id,
					segmentIndex,
					branchName,
					branchRef,
					commitId: commit.id,
				});
				indexByKey.set(itemKey(commitItem), items.length);
				sectionIndexByItemIndex.push(sectionIndex);
				items.push(commitItem);
			}
		}
	}

	return { items, sections, sectionIndexByItemIndex, indexByKey };
};

export const getAdjacentItem = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	const currentIndex = selection ? (model.indexByKey.get(itemKey(selection)) ?? -1) : -1;
	if (currentIndex === -1) return null;
	const itemCount = model.items.length;
	if (itemCount === 0) return null;
	return model.items[(currentIndex + offset + itemCount) % itemCount] ?? null;
};

export const getAdjacentSection = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = model.indexByKey.get(itemKey(selection));
	if (currentIndex === undefined) return null;
	const currentSectionIndex = model.sectionIndexByItemIndex[currentIndex] ?? -1;
	if (currentSectionIndex === -1) return null;
	const sectionCount = model.sections.length;
	if (sectionCount === 0) return null;
	return model.sections[(currentSectionIndex + offset + sectionCount) % sectionCount] ?? null;
};

export type SharedSelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "NextSection" }
	| { _tag: "PreviousSection" };

type ChangesSelectionAction = SharedSelectionAction;
type SegmentSelectionAction = SharedSelectionAction;
type CommitSelectionAction =
	| SharedSelectionAction
	| { _tag: "EditCommitMessage" }
	| { _tag: "ExpandCommit" };
type CommitEditingMessageAction = { _tag: "Save" } | { _tag: "Cancel" };

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
		id: "next-section",
		description: "next section",
		keys: ["J"],
		action: { _tag: "NextSection" },
	},
	{
		id: "previous-section",
		description: "previous section",
		keys: ["K"],
		action: { _tag: "PreviousSection" },
	},
];

export const changesSelectionBindings: Array<ShortcutBinding<ChangesSelectionAction, ChangesItem>> =
	[
		...createSharedSelectionBindings<ChangesItem>(),
		{
			id: "changes-previous-section",
			description: "previous section",
			keys: ["ArrowLeft"],
			action: { _tag: "PreviousSection" },
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

export const commitEditingMessageBindings: Array<ShortcutBinding<CommitEditingMessageAction>> = [
	{
		id: "commit-editing-message-save",
		description: "save",
		keys: ["Enter"],
		action: { _tag: "Save" },
		repeat: false,
	},
	{
		id: "commit-editing-message-cancel",
		description: "cancel",
		keys: ["Escape"],
		action: { _tag: "Cancel" },
		repeat: false,
	},
];
