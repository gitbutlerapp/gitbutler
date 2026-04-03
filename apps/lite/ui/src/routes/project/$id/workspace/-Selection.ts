import {
	DiffHunk,
	HunkHeader,
	Segment,
	type HunkAssignment,
	type RefInfo,
	type TreeChange,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	baseCommitItem,
	changesDetailsItem,
	changesSummaryItem,
	detailsFileItem,
	type Item,
	segmentItem,
	commitItem,
} from "./-Item.ts";
import { hunkHeaderEquals } from "../-shared.tsx";

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

const navigationItemKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			Changes: (item) =>
				item.mode._tag === "Details"
					? JSON.stringify(["Changes", item.stackId, "Details", item.mode.item.path])
					: JSON.stringify(["Changes", item.stackId, item.mode._tag]),
			Segment: (item) => JSON.stringify(["Segment", item.stackId, item.segmentIndex]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.segmentIndex, item.commitId]),
			BaseCommit: (item) => JSON.stringify(["BaseCommit", item.commitId]),
		}),
	);

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
		model.indexByKey.set(navigationItemKey(item), model.items.length);
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
			addItem(changesDetailsItem(stackId, detailsFileItem(change.path)), sectionIndex);
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
	const currentIndex = model.indexByKey.get(navigationItemKey(selection));
	if (currentIndex === undefined) return null;
	return getRelative(model.items, currentIndex, offset);
};

export const getAdjacentSection = (
	model: NavigationModel,
	selection: Item | null,
	offset: -1 | 1,
): Item | null => {
	if (!selection) return null;
	const currentIndex = model.indexByKey.get(navigationItemKey(selection));
	if (currentIndex === undefined) return null;
	const currentSectionIndex = model.sectionIndexByItemIndex[currentIndex] ?? -1;
	if (currentSectionIndex === -1) return null;
	return getRelative(model.sections, currentSectionIndex, offset);
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

export const getAdjacentHunk = ({
	hunks,
	currentHunk,
	offset,
}: {
	hunks: Array<DiffHunk>;
	currentHunk: HunkHeader | undefined;
	offset: -1 | 1;
}): DiffHunk | null => {
	if (hunks.length === 0) return null;
	if (currentHunk === undefined) return offset > 0 ? (hunks[0] ?? null) : (hunks.at(-1) ?? null);

	const currentIndex = hunks.findIndex((hunk) => hunkHeaderEquals(hunk, currentHunk));
	if (currentIndex === -1) return offset > 0 ? (hunks[0] ?? null) : (hunks.at(-1) ?? null);
	return hunks[currentIndex + offset] ?? null;
};
