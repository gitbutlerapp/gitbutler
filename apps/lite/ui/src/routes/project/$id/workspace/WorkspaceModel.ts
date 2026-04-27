import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { CommitDetails, Segment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { useQueries, useSuspenseQuery } from "@tanstack/react-query";
import { type NonEmptyArray } from "effect/Array";
import { changesFileParent, commitFileParent } from "#ui/domain/FileParent.ts";
import {
	branchItem,
	baseCommitItem,
	changesSectionItem,
	type Item,
	commitItem,
	fileItem,
	itemIdentityKey,
	stackItem,
} from "./Item.ts";

type WorkspaceSection = {
	section: Item | null;
	children: Array<Item>;
};

type WorkspaceOutline = NonEmptyArray<WorkspaceSection>;

type BuildWorkspaceOutlineArgs = {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	expandedCommitDetails?: CommitDetails;
};

const buildWorkspaceOutline = ({
	headInfo,
	changes,
	expandedCommitDetails,
}: BuildWorkspaceOutlineArgs): WorkspaceOutline => {
	const changesSection: WorkspaceSection = {
		section: changesSectionItem,
		children: changes.map((change) => fileItem({ parent: changesFileParent, path: change.path })),
	};

	const segmentChildren = (stackId: string, segment: Segment): Array<Item> =>
		segment.commits.flatMap(
			(commit): Array<Item> => [
				commitItem({ stackId, commitId: commit.id }),
				...(commit.id === expandedCommitDetails?.commit.id
					? expandedCommitDetails.changes.map((change) =>
							fileItem({
								parent: commitFileParent({ stackId, commitId: commit.id }),
								path: change.path,
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
		changesSection,

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
	const commitDetailsQueries = useQueries({
		queries: (expandedCommitId !== null ? [expandedCommitId] : []).map((commitId) =>
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		),
	});

	return buildWorkspaceOutline({
		headInfo,
		changes: worktreeChanges.changes,
		expandedCommitDetails: commitDetailsQueries[0]?.data,
	});
};

export type NavigationIndex = {
	items: Array<Item>;
	sectionStartIndexes: Array<number>;
	sectionIndexByItemIndex: Array<number>;
	indexByKey: Map<string, number>;
};

const createNavigationIndex = (): NavigationIndex => ({
	items: [],
	sectionStartIndexes: [],
	sectionIndexByItemIndex: [],
	indexByKey: new Map<string, number>(),
});

export const buildNavigationIndex = (outline: WorkspaceOutline): NavigationIndex => {
	const index = createNavigationIndex();

	for (const outlineSection of outline) {
		const itemsInSection = outlineSection.section
			? [outlineSection.section, ...outlineSection.children]
			: outlineSection.children;
		if (itemsInSection.length === 0) continue;

		const sectionIndex = index.sectionStartIndexes.length;
		index.sectionStartIndexes.push(index.items.length);

		for (const item of itemsInSection) {
			const itemIndex = index.items.length;
			index.items.push(item);
			index.sectionIndexByItemIndex.push(sectionIndex);
			index.indexByKey.set(itemIdentityKey(item), itemIndex);
		}
	}

	return index;
};

export const filterNavigationIndex = (
	index: NavigationIndex,
	predicate: (item: Item) => boolean,
): NavigationIndex => {
	const filteredIndex = createNavigationIndex();

	const sectionIndexBySourceSectionIndex = new Map<number, number>();

	for (const [itemIndex, item] of index.items.entries()) {
		if (!predicate(item)) continue;

		const sourceSectionIndex = index.sectionIndexByItemIndex[itemIndex];
		if (sourceSectionIndex === undefined) continue;
		let filteredSectionIndex = sectionIndexBySourceSectionIndex.get(sourceSectionIndex);
		if (filteredSectionIndex === undefined) {
			filteredSectionIndex = filteredIndex.sectionStartIndexes.length;
			sectionIndexBySourceSectionIndex.set(sourceSectionIndex, filteredSectionIndex);
			filteredIndex.sectionStartIndexes.push(filteredIndex.items.length);
		}

		const filteredItemIndex = filteredIndex.items.length;
		filteredIndex.items.push(item);
		filteredIndex.sectionIndexByItemIndex.push(filteredSectionIndex);
		filteredIndex.indexByKey.set(itemIdentityKey(item), filteredItemIndex);
	}

	return filteredIndex;
};

export const getAdjacent = ({
	navigationIndex,
	selectedItem,
	offset,
}: {
	navigationIndex: NavigationIndex;
	selectedItem: Item;
	offset: -1 | 1;
}): Item | null => {
	const selectedItemIndex = navigationIndex.indexByKey.get(itemIdentityKey(selectedItem));
	if (selectedItemIndex === undefined) return null;

	return navigationIndex.items[selectedItemIndex + offset] ?? null;
};

export const getNextSection = ({
	navigationIndex,
	selectedItem,
}: {
	navigationIndex: NavigationIndex;
	selectedItem: Item;
}): Item | null => {
	const selectedItemIndex = navigationIndex.indexByKey.get(itemIdentityKey(selectedItem));
	if (selectedItemIndex === undefined) return null;

	const sectionIndex = navigationIndex.sectionIndexByItemIndex[selectedItemIndex];
	if (sectionIndex === undefined) return null;
	const nextSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex + 1];
	if (nextSectionStartIndex === undefined) return null;

	return navigationIndex.items[nextSectionStartIndex] ?? null;
};

export const getPreviousSection = ({
	navigationIndex,
	selectedItem,
}: {
	navigationIndex: NavigationIndex;
	selectedItem: Item;
}): Item | null => {
	const selectedItemIndex = navigationIndex.indexByKey.get(itemIdentityKey(selectedItem));
	if (selectedItemIndex === undefined) return null;

	const sectionIndex = navigationIndex.sectionIndexByItemIndex[selectedItemIndex];
	if (sectionIndex === undefined) return null;
	const currentSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex];
	if (currentSectionStartIndex === undefined) return null;

	if (selectedItemIndex !== currentSectionStartIndex)
		return navigationIndex.items[currentSectionStartIndex] ?? null;

	const previousSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex - 1];
	if (previousSectionStartIndex === undefined) return null;

	return navigationIndex.items[previousSectionStartIndex] ?? null;
};

export const navigationIndexIncludes = (navigationIndex: NavigationIndex, item: Item): boolean =>
	navigationIndex.indexByKey.has(itemIdentityKey(item));
