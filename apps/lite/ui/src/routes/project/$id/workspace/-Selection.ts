import { Segment, type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import {
	baseCommitItem,
	changesDetailsItem,
	changesSummaryItem,
	itemKey,
	type Item,
	segmentItem,
	commitItem,
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

export type NavigationModel = {
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
	const model: NavigationModel = {
		items: [],
		sections: [],
		sectionIndexByItemIndex: [],
		indexByKey: new Map<string, number>(),
	};

	const addItem = (item: Item, sectionIndex: number) => {
		model.indexByKey.set(itemKey(item), model.items.length);
		model.sectionIndexByItemIndex.push(sectionIndex);
		model.items.push(item);
	};

	const addSection = (section: Item) => {
		const sectionIndex = model.sections.length;
		model.sections.push(section);
		addItem(section, sectionIndex);
	};

	const addChangesSection = (stackId: string | null) => {
		const sectionIndex = model.sections.length;
		addSection(changesSummaryItem(stackId));

		for (const change of changes) {
			if (!hasAssignmentsForPath({ assignments, stackId, path: change.path })) continue;
			addItem(changesDetailsItem(stackId, change.path), sectionIndex);
		}
	};

	const addSegmentSection = (stackId: string, segmentIndex: number, segment: Segment) => {
		const branchName = segment.refName?.displayName ?? null;
		const sectionIndex = model.sections.length;
		addSection(segmentItem({ stackId, segmentIndex, branchName }));

		for (const commit of segment.commits)
			addItem(commitItem({ stackId, segmentIndex, branchName, commitId: commit.id }), sectionIndex);
	};

	addChangesSection(null);

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;
		const stackId = stack.id;
		addChangesSection(stackId);

		for (const [segmentIndex, segment] of stack.segments.entries())
			addSegmentSection(stackId, segmentIndex, segment);
	}

	if (commonBaseCommitId !== undefined) addSection(baseCommitItem(commonBaseCommitId));

	return model;
};

const getRelative = <T>(items: Array<T>, index: number, offset: -1 | 1): T | null => {
	const itemCount = items.length;
	if (itemCount === 0) return null;
	return items[(index + offset + itemCount) % itemCount] ?? null;
};

export const getAdjacentItem = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = model.indexByKey.get(itemKey(selection));
	if (currentIndex === undefined) return null;
	return getRelative(model.items, currentIndex, offset);
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
	return getRelative(model.sections, currentSectionIndex, offset);
};

export const getAdjacentCommitDetailsPath = ({
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

export const getSelectedCommitPath = ({
	paths,
	selection,
}: {
	paths: Array<string>;
	selection: CommitItem;
}): string | undefined =>
	selection.mode._tag === "Details" &&
	selection.mode.path !== undefined &&
	paths.includes(selection.mode.path)
		? selection.mode.path
		: paths[0];
