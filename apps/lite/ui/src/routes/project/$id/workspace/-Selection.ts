import { getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { type ShortcutBinding } from "#ui/shortcuts.ts";
import { Match } from "effect";
import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	baseCommitItem,
	changesDetailsItem,
	changesSummaryItem,
	itemKey,
	type Item,
	segmentItem,
	commitItem,
	ChangesItem,
	CommitItem,
} from "./-Item.ts";
import { type EditingCommit } from "./-EditingCommit.ts";
import { ShortcutsBarMode } from "./-ShortcutsBar.tsx";

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
	commonBaseCommitId,
}: {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	commonBaseCommitId?: string;
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
				const commitItemV = commitItem({
					stackId: stack.id,
					segmentIndex,
					branchName,
					branchRef,
					commitId: commit.id,
				});
				indexByKey.set(itemKey(commitItemV), items.length);
				sectionIndexByItemIndex.push(sectionIndex);
				items.push(commitItemV);
			}
		}
	}

	if (commonBaseCommitId !== undefined) {
		const section = baseCommitItem(commonBaseCommitId);
		const sectionIndex = sections.length;
		sections.push(section);
		indexByKey.set(itemKey(section), items.length);
		sectionIndexByItemIndex.push(sectionIndex);
		items.push(section);
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
type CommitSummarySelectionAction =
	| SharedSelectionAction
	| { _tag: "EditCommitMessage" }
	| { _tag: "ExpandCommit" };
type CommitDetailsSelectionAction = SharedSelectionAction | { _tag: "CloseCommitDetails" };
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

export const commitSummarySelectionBindings: Array<
	ShortcutBinding<CommitSummarySelectionAction, CommitItem>
> = [
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

export const commitDetailsSelectionBindings: Array<
	ShortcutBinding<CommitDetailsSelectionAction, CommitItem>
> = [
	...createSharedSelectionBindings<CommitItem>(),
	{
		id: "commit-close-details",
		description: "close details",
		keys: ["ArrowLeft", "Escape"],
		action: { _tag: "CloseCommitDetails" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Details",
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

export const getShortcutsBarMode = ({
	selection,
	editingCommit,
}: {
	selection: Item | null;
	editingCommit: EditingCommit | null;
}): ShortcutsBarMode | null => {
	if (selection === null) return null;

	return Match.value(selection).pipe(
		Match.tag(
			"Changes",
			(selection): ShortcutsBarMode => ({
				label: "changes",
				items: changesSelectionBindings.filter((binding) => binding.when?.(selection) ?? true),
			}),
		),
		Match.tag("Commit", (selection): ShortcutsBarMode => {
			if (
				editingCommit !== null &&
				editingCommit.stackId === selection.stackId &&
				editingCommit.segmentIndex === selection.segmentIndex &&
				editingCommit.commitId === selection.commitId
			)
				return {
					label: "edit message",
					items: commitEditingMessageBindings,
				};

			return Match.value(selection.mode).pipe(
				Match.tag(
					"Details",
					(): ShortcutsBarMode => ({
						label: "commit details",
						items: commitDetailsSelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.tag(
					"Summary",
					(): ShortcutsBarMode => ({
						label: "commit",
						items: commitSummarySelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.exhaustive,
			);
		}),
		Match.tag(
			"BaseCommit",
			(): ShortcutsBarMode => ({
				label: "base commit",
				items: segmentSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
			}),
		),
		Match.tag(
			"Segment",
			(): ShortcutsBarMode => ({
				label: "segment",
				items: segmentSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
			}),
		),
		Match.exhaustive,
	);
};
