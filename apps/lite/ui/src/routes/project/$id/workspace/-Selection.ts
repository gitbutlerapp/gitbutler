import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
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
			const section = segmentItem({
				stackId: stack.id,
				segmentIndex,
				branchName,
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
	if (!selection) return null;
	const currentIndex = model.indexByKey.get(itemKey(selection));
	if (currentIndex === undefined) return null;
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
