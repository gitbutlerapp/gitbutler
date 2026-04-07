import { Segment, type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { type NonEmptyArray } from "effect/Array";
import {
	baseCommitItem,
	changeItem,
	changesSectionItem,
	type Item,
	commitItem,
	itemIdentityKey,
	segmentItem,
} from "./-Item.ts";
import { getRelative } from "../-shared.tsx";
import { asSelectedItem, SelectedItem } from "./-SelectedItem.ts";

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

type WorkspaceSection = {
	section: Item;
	children: Array<Item>;
};

type WorkspaceOutline = NonEmptyArray<WorkspaceSection>;

type BuildWorkspaceOutlineArgs = {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	commonBaseCommitId?: string;
};

export const buildWorkspaceOutline = ({
	headInfo,
	changes,
	assignments,
	commonBaseCommitId,
}: BuildWorkspaceOutlineArgs): WorkspaceOutline => {
	const changesSection = (stackId: string | null): WorkspaceSection => ({
		section: changesSectionItem(stackId),
		children: changes.flatMap((change) =>
			hasAssignmentsForPath({ assignments, stackId, path: change.path })
				? [changeItem(stackId, change.path)]
				: [],
		),
	});

	const segmentSection = (
		stackId: string,
		segmentIndex: number,
		segment: Segment,
	): WorkspaceSection => {
		const branchName = segment.refName?.displayName ?? null;
		return {
			section: segmentItem({ stackId, segmentIndex, branchName }),
			children: segment.commits.map((commit) =>
				commitItem({ stackId, segmentIndex, branchName, commitId: commit.id }),
			),
		};
	};

	const baseCommitSection = (commitId: string): WorkspaceSection => ({
		section: baseCommitItem(commitId),
		children: [],
	});

	return [
		changesSection(null),

		...headInfo.stacks.flatMap((stack) => {
			if (stack.id == null) return [];
			const stackId = stack.id;
			return [
				changesSection(stackId),
				...stack.segments.map((segment, segmentIndex) =>
					segmentSection(stackId, segmentIndex, segment),
				),
			];
		}),

		...(commonBaseCommitId !== undefined ? [baseCommitSection(commonBaseCommitId)] : []),
	];
};

export type NavigationIndex = {
	items: Array<Item>;
	sectionStartIndexes: Array<number>;
	sectionIndexByItemIndex: Array<number>;
	indexByKey: Map<string, number>;
};

export const buildNavigationIndex = (outline: WorkspaceOutline): NavigationIndex => {
	const model: NavigationIndex = {
		items: [],
		sectionStartIndexes: [],
		sectionIndexByItemIndex: [],
		indexByKey: new Map<string, number>(),
	};

	const addItem = (item: Item, sectionIndex: number) => {
		model.indexByKey.set(itemIdentityKey(item), model.items.length);
		model.sectionIndexByItemIndex.push(sectionIndex);
		model.items.push(item);
	};

	for (const { section, children } of outline) {
		const sectionIndex = model.sectionStartIndexes.length;
		model.sectionStartIndexes.push(model.items.length);
		addItem(section, sectionIndex);

		for (const item of children) addItem(item, sectionIndex);
	}

	return model;
};

export const getAdjacentItem = (
	index: NavigationIndex,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = index.indexByKey.get(itemIdentityKey(selection));
	if (currentIndex === undefined) return null;
	return getRelative(index.items, currentIndex, offset);
};

export const getAdjacentSection = (
	index: NavigationIndex,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = index.indexByKey.get(itemIdentityKey(selection));
	if (currentIndex === undefined) return null;
	const currentSectionIndex = index.sectionIndexByItemIndex[currentIndex] ?? -1;
	if (currentSectionIndex === -1) return null;
	const adjacentSectionStartIndex = getRelative(
		index.sectionStartIndexes,
		currentSectionIndex,
		offset,
	);
	if (adjacentSectionStartIndex === null) return null;
	return index.items[adjacentSectionStartIndex] ?? null;
};

export const navigationIndexIncludes = (navigationIndex: NavigationIndex, item: Item): boolean =>
	navigationIndex.indexByKey.has(itemIdentityKey(item));

export const getDefaultSelectedItem = (navigationIndex: NavigationIndex): SelectedItem | null =>
	navigationIndex.items[0] ? asSelectedItem(navigationIndex.items[0]) : null;
