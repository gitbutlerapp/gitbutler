import { Segment, type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	baseCommitItem,
	changeItem,
	changesSectionItem,
	type Item,
	segmentItem,
	commitItem,
} from "./-Item.ts";
import { getRelative } from "../-shared.tsx";
import { Match } from "effect";

// We intentionally omit state like mode, which affects presentation but not
// navigation.
const navigationItemKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			Changes: (item) => JSON.stringify(["Changes", item.stackId]),
			Change: (item) => JSON.stringify(["Change", item.stackId, item.path]),
			Segment: (item) =>
				JSON.stringify(["Segment", item.stackId, item.segmentIndex, item.branchName]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.segmentIndex, item.commitId]),
			BaseCommit: (item) => JSON.stringify(["BaseCommit", item.commitId]),
		}),
	);

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

export type WorkspaceSection = {
	section: Item;
	items: Array<Item>;
};

export type WorkspaceOutline = Array<WorkspaceSection>;

export const buildWorkspaceOutline = ({
	headInfo,
	changes,
	assignments,
	commonBaseCommitId,
}: {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	commonBaseCommitId?: string;
}): WorkspaceOutline => {
	const changesSection = (stackId: string | null): WorkspaceSection => ({
		section: changesSectionItem(stackId),
		items: changes.flatMap((change) =>
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
			items: segment.commits.map((commit) =>
				commitItem({ stackId, segmentIndex, branchName, commitId: commit.id }),
			),
		};
	};

	const baseCommitSection = (commitId: string): WorkspaceSection => ({
		section: baseCommitItem(commitId),
		items: [],
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
	sections: Array<Item>;
	sectionIndexByItemIndex: Array<number>;
	indexByKey: Map<string, number>;
};

export const buildNavigationIndex = (outline: WorkspaceOutline): NavigationIndex => {
	const model: NavigationIndex = {
		items: [],
		sections: [],
		sectionIndexByItemIndex: [],
		indexByKey: new Map<string, number>(),
	};

	const addItem = (item: Item, sectionIndex: number) => {
		model.indexByKey.set(navigationItemKey(item), model.items.length);
		model.sectionIndexByItemIndex.push(sectionIndex);
		model.items.push(item);
	};

	for (const { section, items } of outline) {
		const sectionIndex = model.sections.length;
		model.sections.push(section);
		addItem(section, sectionIndex);

		for (const item of items) addItem(item, sectionIndex);
	}

	return model;
};

export const getAdjacentItem = (
	index: NavigationIndex,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = index.indexByKey.get(navigationItemKey(selection));
	if (currentIndex === undefined) return null;
	return getRelative(index.items, currentIndex, offset);
};

export const getAdjacentSection = (
	index: NavigationIndex,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = index.indexByKey.get(navigationItemKey(selection));
	if (currentIndex === undefined) return null;
	const currentSectionIndex = index.sectionIndexByItemIndex[currentIndex] ?? -1;
	if (currentSectionIndex === -1) return null;
	return getRelative(index.sections, currentSectionIndex, offset);
};

export const getAdjacentPath = ({
	paths,
	currentPath,
	offset,
}: {
	paths: Array<string>;
	currentPath: string | undefined;
	offset: -1 | 1;
}): string | null => {
	if (paths.length === 0) return null;
	if (currentPath === undefined) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);

	const currentIndex = paths.indexOf(currentPath);
	if (currentIndex === -1) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);
	return paths[currentIndex + offset] ?? null;
};

export const normalizeSelectedFile = ({
	paths,
	selectedFile,
}: {
	paths: Array<string>;
	selectedFile: string | null | undefined;
}): string | undefined => {
	if (selectedFile != null && paths.includes(selectedFile)) return selectedFile;
	return paths[0];
};

export const normalizeSelectedHunk = ({
	hunkKeys,
	selectedHunk,
}: {
	hunkKeys: Array<string>;
	selectedHunk: string | null;
}): string | undefined => {
	if (selectedHunk !== null && hunkKeys.includes(selectedHunk)) return selectedHunk;
	return hunkKeys[0];
};
