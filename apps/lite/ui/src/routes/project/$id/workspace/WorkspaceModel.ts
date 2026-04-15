import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { Segment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { type NonEmptyArray } from "effect/Array";
import {
	branchItem,
	baseCommitItem,
	changeItem,
	changesSectionItem,
	type Item,
	commitItem,
	commitFileItem,
	itemEquals,
	itemIdentityKey,
	stackItem,
} from "./Item.ts";
import { getRelative } from "../shared.tsx";

type WorkspaceSection = {
	section: Item | null;
	children: Array<Item>;
};

type WorkspaceOutline = NonEmptyArray<WorkspaceSection>;

type BuildWorkspaceOutlineArgs = {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	expandedCommitId?: string | null;
	expandedCommitPaths?: Array<string>;
};

const buildWorkspaceOutline = ({
	headInfo,
	changes,
	expandedCommitId = null,
	expandedCommitPaths,
}: BuildWorkspaceOutlineArgs): WorkspaceOutline => {
	const changesSection = (stackId: string | null): WorkspaceSection => ({
		section: changesSectionItem({ stackId }),
		children: changes.map((change) => changeItem({ path: change.path })),
	});

	const segmentChildren = (stackId: string, segment: Segment): Array<Item> =>
		segment.commits.flatMap(
			(commit): Array<Item> => [
				commitItem({ stackId, commitId: commit.id }),
				...(commit.id === expandedCommitId
					? (expandedCommitPaths ?? []).map((path) =>
							commitFileItem({
								stackId,
								commitId: commit.id,
								path,
							}),
						)
					: []),
			],
		);

	const segmentSection = (stackId: string, segment: Segment): WorkspaceSection | null => {
		const children = segmentChildren(stackId, segment);
		const branchRef = segment.refName?.fullNameBytes;
		if (!branchRef && children.length === 0) return null;

		return {
			section: branchRef ? branchItem({ stackId, branchRef }) : null,
			children,
		};
	};

	const baseCommitSection: WorkspaceSection = {
		section: baseCommitItem,
		children: [],
	};

	return [
		changesSection(null),

		...headInfo.stacks.flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			const stackItemSection: WorkspaceSection = {
				section: stackItem({ stackId }),
				children: [],
			};
			return [
				stackItemSection,
				...stack.segments.flatMap((segment) => {
					const section = segmentSection(stackId, segment);
					return section ? [section] : [];
				}),
			];
		}),

		baseCommitSection,
	];
};

export const useWorkspaceOutline = ({
	projectId,
	expandedCommitId,
}: {
	projectId: string;
	expandedCommitId: string | null;
}) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const { data: expandedCommitDetails } = useQuery({
		...commitDetailsWithLineStatsQueryOptions({
			projectId,
			commitId: expandedCommitId ?? "",
		}),
		enabled: expandedCommitId !== null,
	});

	return buildWorkspaceOutline({
		headInfo,
		changes: worktreeChanges.changes,
		expandedCommitId,
		expandedCommitPaths: expandedCommitDetails?.changes.map((change) => change.path) ?? [],
	});
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
		if (section) addItem(section, sectionIndex);

		for (const item of children) addItem(item, sectionIndex);
	}

	return model;
};

export const filterNavigationIndex = (
	index: NavigationIndex,
	predicate: (item: Item) => boolean,
): NavigationIndex => {
	const filteredIndex: NavigationIndex = {
		items: [],
		sectionStartIndexes: [],
		sectionIndexByItemIndex: [],
		indexByKey: new Map<string, number>(),
	};

	let currentSectionIndex = -1;
	let filteredSectionIndex = -1;

	for (const [itemIndex, item] of index.items.entries()) {
		if (!predicate(item)) continue;

		const sectionIndex = index.sectionIndexByItemIndex[itemIndex] ?? -1;
		if (sectionIndex !== currentSectionIndex) {
			// Preserve the original section grouping, even when the section header
			// itself is filtered out and the retained item is one of its children.
			currentSectionIndex = sectionIndex;
			filteredSectionIndex = filteredIndex.sectionStartIndexes.length;
			filteredIndex.sectionStartIndexes.push(filteredIndex.items.length);
		}

		filteredIndex.indexByKey.set(itemIdentityKey(item), filteredIndex.items.length);
		filteredIndex.sectionIndexByItemIndex.push(filteredSectionIndex);
		filteredIndex.items.push(item);
	}

	return filteredIndex;
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

const getCurrentSection = (index: NavigationIndex, selection: Item | null): Item | null => {
	if (!selection) return null;
	const currentIndex = index.indexByKey.get(itemIdentityKey(selection));
	if (currentIndex === undefined) return null;
	const currentSectionIndex = index.sectionIndexByItemIndex[currentIndex] ?? -1;
	if (currentSectionIndex === -1) return null;
	const currentSectionStartIndex = index.sectionStartIndexes[currentSectionIndex];
	if (currentSectionStartIndex === undefined) return null;
	return index.items[currentSectionStartIndex] ?? null;
};

export const getAdjacentSection = (
	index: NavigationIndex,
	selectedItem: Item | null,
	offset: -1 | 1,
): Item | null => {
	const currentSection = getCurrentSection(index, selectedItem);
	if (currentSection === null) return null;
	const currentIndex = index.indexByKey.get(itemIdentityKey(currentSection));
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

export const getPreviousSection = (
	index: NavigationIndex,
	selectedItem: Item | null,
): Item | null => {
	const currentSection = getCurrentSection(index, selectedItem);
	if (currentSection === null) return null;
	return selectedItem !== null && !itemEquals(currentSection, selectedItem)
		? currentSection
		: getAdjacentSection(index, currentSection, -1);
};

export const navigationIndexIncludes = (navigationIndex: NavigationIndex, item: Item): boolean =>
	navigationIndex.indexByKey.has(itemIdentityKey(item));

export const getDefaultItem = (navigationIndex: NavigationIndex): Item | null =>
	navigationIndex.items[0] ?? null;
