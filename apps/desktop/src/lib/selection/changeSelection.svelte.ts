import { previousPathBytesFromTreeChange, type TreeChange } from '$lib/hunks/change';
import {
	hunkGroupToKey,
	type HunkAssignments,
	type HunkGroup
} from '$lib/hunks/diffService.svelte';
import { hunkHeaderEquals, type HunkAssignment } from '$lib/hunks/hunk';
import { createSelectByPrefix } from '$lib/state/customSelectors';
import { type Reactive, reactive } from '@gitbutler/shared/storeUtils';
import { type LineId } from '@gitbutler/ui/utils/diffParsing';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';

export class HunkSelection {
	private state = $state([]);
}

type HunkHeader = {
	oldStart: number;
	oldLines: number;
	newStart: number;
	newLines: number;
};

export type FullySelectedHunk = HunkHeader & {
	type: 'full';
};

export type PartiallySelectedHunk = HunkHeader & {
	type: 'partial';
	lines: LineId[];
};

/**
 * Representation of visually selected hunk.
 */
export type SelectedHunk = FullySelectedHunk | PartiallySelectedHunk;

type FileHeader = {
	path: string;
	pathBytes: number[];
	previousPathBytes: number[] | null;
};

export type FullySelectedFile = FileHeader & {
	type: 'full';
};

export type PartiallySelectedFile = FileHeader & {
	type: 'partial';
	hunks: SelectedHunk[];
};

/**
 * Representation of visually selected file.
 */
export type SelectedFile = FullySelectedFile | PartiallySelectedFile;

export const changeSelectionAdapter = createEntityAdapter<SelectedFile, SelectedFile['path']>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => a.path.localeCompare(b.path)
});

const { selectById, selectAll } = changeSelectionAdapter.getSelectors();
const selectByPrefix = createSelectByPrefix<SelectedFile>();

export const changeSelectionSlice = createSlice({
	name: 'changeSelection',
	initialState: changeSelectionAdapter.getInitialState(),
	reducers: {
		addOne: changeSelectionAdapter.addOne,
		addMany: changeSelectionAdapter.addMany,
		removeOne: changeSelectionAdapter.removeOne,
		removeMany: changeSelectionAdapter.removeMany,
		removeAll: changeSelectionAdapter.removeAll,
		upsertOne: changeSelectionAdapter.upsertOne
	},
	selectors: { selectById, selectAll }
});

const { addMany, removeOne, removeMany, removeAll, upsertOne } = changeSelectionSlice.actions;

function sortHunksInFile(file: SelectedFile) {
	if (file.type === 'full') {
		return file;
	}

	const hunks = file.hunks.slice().sort((a, b) => a.newStart - b.newStart);
	return { ...file, hunks };
}

export class ChangeSelectionService {
	/** The change selection slice of the full redux state. */
	private state = $state<EntityState<SelectedFile, string>>(changeSelectionSlice.getInitialState());

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		$effect(() => {
			this.state = reactiveState.current;
		});
	}

	list(): Reactive<SelectedFile[]> {
		const selected = $derived(selectAll(this.state));
		return reactive(() => selected);
	}

	getById(path: string): Reactive<SelectedFile | undefined> {
		const selected = $derived(selectById(this.state, path));
		return reactive(() => selected);
	}

	getByPrefix(path: string): Reactive<SelectedFile[]> {
		const selected = $derived(selectByPrefix(this.state, path));
		return reactive(() => selected);
	}

	upsert(file: SelectedFile) {
		this.dispatch(upsertOne(file));
	}

	addMany(files: SelectedFile[]) {
		this.dispatch(addMany(files));
	}

	update(file: SelectedFile) {
		const sortedFile = sortHunksInFile(file);
		this.dispatch(upsertOne(sortedFile));
	}

	remove(path: string) {
		this.dispatch(removeOne(path));
	}

	/** Clears any selected items that are not in `paths`.  */
	retain(paths: string[] | undefined) {
		if (paths === undefined) {
			this.dispatch(removeAll());
			return;
		}
		const selection = selectAll(this.state);
		const expired = [];
		for (const change of selection) {
			if (!paths.includes(change.path)) {
				expired.push(change.path);
			}
		}
		if (expired.length > 0) {
			this.dispatch(removeMany(expired));
		}
	}

	every(paths: string[], predicate: (selection: SelectedFile) => boolean): boolean {
		const selection = selectAll(this.state);
		for (const path of paths) {
			const change = selection.find((change) => change.path === path);
			if (change === undefined || !predicate(change)) {
				return false;
			}
		}
		return true;
	}

	clear() {
		this.dispatch(removeAll());
	}
}

/**
 * Takes the assignments from a given path and returns whether they all have
 * header information.
 */
function assignmentsHaveHunkInformation(
	assignments: HunkAssignment[]
): assignments is (HunkAssignment & { hunkHeader: HunkHeader })[] {
	// We only need to do "some" because an invariant from the backend is
	// that if one assignment for a given path has a header, they all will
	// have headers
	return assignments.some((assignment) => isDefined(assignment.hunkHeader));
}

/**
 * Intended behaviour:
 * - If the user has selected a given stack when they start commiting
 *   - If the stack has assigned changes
 *     - Select those
 * - Otherwise
 *   - Select the uncommited changes
 */
export function selectForStartingCommit(
	stackId: string | undefined,
	changes: TreeChange[],
	assignments: HunkAssignments,
	currentlySelectedFiles: SelectedFile[],
	changeSelection: ChangeSelectionService
) {
	if (currentlySelectedFiles.length > 0) return;

	let group: HunkGroup;
	if (
		stackId &&
		changes.some(
			(change) =>
				getRelevantAssignments(change, { type: 'grouped', stackId }, assignments).length > 0
		)
	) {
		group = { type: 'grouped', stackId };
	} else {
		group = { type: 'ungrouped' };
	}

	for (const change of changes) {
		selectAllForChangeInGroup(change, group, assignments, undefined, changeSelection);
	}
}

export function selectAllForChangeInGroup(
	change: TreeChange,
	group: HunkGroup,
	assignments: HunkAssignments,
	existingSelection: SelectedFile | undefined,
	changeSelection: ChangeSelectionService
) {
	if (existingSelection?.type === 'full') return;

	const relevantAssignments = getRelevantAssignments(change, group, assignments);
	if (relevantAssignments.length === 0) return;

	// If there are any relevant assignments without a hunk header, it means
	// that the assignment itself represents a whole file, like a rename,
	// deletion, or addition. In this case we can add the entire file as
	// there should only be one assignment for the file.
	if (!assignmentsHaveHunkInformation(relevantAssignments)) {
		changeSelection.upsert({
			type: 'full',
			path: change.path,
			pathBytes: change.pathBytes,
			previousPathBytes: previousPathBytesFromTreeChange(change)
		});
		return;
	}

	const allAssignmentsExceptRelevant = getAllAssignments(change, assignments, group);

	if (!existingSelection) {
		// There is no existing selection so we can simply select all the
		// hunks belonging to the group, making sure to use type full if it
		// turns out that all the hunks are assigned to the current group.
		if (allAssignmentsExceptRelevant.length === 0) {
			changeSelection.upsert({
				type: 'full',
				path: change.path,
				pathBytes: change.pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change)
			});
		} else {
			changeSelection.upsert({
				type: 'partial',
				path: change.path,
				pathBytes: change.pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change),
				hunks: relevantAssignments.map((assignment) => ({
					...assignment.hunkHeader,
					type: 'full'
				}))
			});
		}
		return;
	}

	// The existingSelection is now present and type === "partial"

	if (allAssignmentsExceptRelevant.length === 0) {
		changeSelection.upsert({
			type: 'full',
			path: change.path,
			pathBytes: change.pathBytes,
			previousPathBytes: previousPathBytesFromTreeChange(change)
		});
	} else {
		// There are some existing selections.
		const currentSelectedHunksWithoutRelevant = existingSelection.hunks.filter(
			(hunk) =>
				!relevantAssignments.some((assignments) => hunkHeaderEquals(assignments.hunkHeader, hunk))
		);

		const endsUpFullyAssigned =
			currentSelectedHunksWithoutRelevant.length + relevantAssignments.length ===
			relevantAssignments.length + allAssignmentsExceptRelevant.length;
		if (endsUpFullyAssigned) {
			changeSelection.upsert({
				type: 'full',
				path: change.path,
				pathBytes: change.pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change)
			});
		} else {
			const newHunks = [
				...currentSelectedHunksWithoutRelevant,
				...relevantAssignments.map<SelectedHunk>((assignment) => ({
					...assignment.hunkHeader,
					type: 'full'
				}))
			];
			changeSelection.upsert({
				type: 'partial',
				path: change.path,
				pathBytes: change.pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change),
				hunks: newHunks
			});
		}
	}
}

export function deselectAllForChangeInGroup(
	change: TreeChange,
	group: HunkGroup,
	assignments: HunkAssignments,
	existingSelection: SelectedFile | undefined,
	changeSelection: ChangeSelectionService
) {
	if (!existingSelection) return;

	const relevantAssignments = getRelevantAssignments(change, group, assignments);
	const allOtherAssignments = getAllAssignments(change, assignments, group);

	// If there are any relevant assignments without a hunk header, it means
	// that the assignment itself represents a whole file, like a rename,
	// deletion, or addition. In this case we can remove the entire file as
	// there should only be one assignment for the file.
	if (
		!assignmentsHaveHunkInformation(relevantAssignments) ||
		!assignmentsHaveHunkInformation(allOtherAssignments)
	) {
		changeSelection.remove(change.path);
		return;
	}

	if (existingSelection.type === 'full') {
		if (allOtherAssignments.length === 0) {
			changeSelection.remove(change.path);
		} else {
			changeSelection.upsert({
				type: 'partial',
				path: change.path,
				pathBytes: change.pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change),
				hunks: allOtherAssignments.map<SelectedHunk>((assignment) => ({
					...assignment.hunkHeader,
					type: 'full'
				}))
			});
		}
		return;
	}

	// existingSelection is partial so we need to filter the hunks
	const remainingHunks = existingSelection.hunks.filter(
		(hunk) =>
			!relevantAssignments.some((assignment) => hunkHeaderEquals(assignment.hunkHeader, hunk))
	);
	if (remainingHunks.length === 0) {
		changeSelection.remove(change.path);
	} else {
		changeSelection.upsert({
			type: 'partial',
			path: change.path,
			pathBytes: change.pathBytes,
			previousPathBytes: previousPathBytesFromTreeChange(change),
			hunks: remainingHunks
		});
	}
}

export function someAssignedToCurrentGroupSelected(
	change: TreeChange,
	group: HunkGroup,
	assignments: HunkAssignments,
	existingSelection: SelectedFile | undefined
): boolean {
	const relevantAssignments = getRelevantAssignments(change, group, assignments);
	if (!existingSelection) return false;
	if (relevantAssignments.length === 0) return false;
	if (existingSelection.type === 'full') return true;
	if (!assignmentsHaveHunkInformation(relevantAssignments)) return true;
	return relevantAssignments.some((assignment) =>
		existingSelection.hunks.some((hunk) => hunkHeaderEquals(hunk, assignment.hunkHeader))
	);
}

export function allAssignedToCurrentGroupSelected(
	change: TreeChange,
	group: HunkGroup,
	assignments: HunkAssignments,
	existingSelection: SelectedFile | undefined
): boolean {
	const relevantAssignments = getRelevantAssignments(change, group, assignments);
	if (!existingSelection) return false;
	if (relevantAssignments.length === 0) return false;
	if (existingSelection.type === 'full') return true;
	if (!assignmentsHaveHunkInformation(relevantAssignments)) return true;
	return relevantAssignments.every((assignment) =>
		existingSelection.hunks.some((hunk) => hunkHeaderEquals(hunk, assignment.hunkHeader))
	);
}

function getRelevantAssignments(
	change: TreeChange,
	group: HunkGroup,
	assignments: HunkAssignments
): HunkAssignment[] {
	const stackGroup = assignments.get(hunkGroupToKey(group));
	if (!stackGroup) return [];
	const hunkAssignments = stackGroup.get(change.path);
	return hunkAssignments ?? [];
}

function getAllAssignments(
	change: TreeChange,
	assignments: HunkAssignments,
	except?: HunkGroup
): HunkAssignment[] {
	const headers = [];

	for (const [key, value] of assignments.entries()) {
		if (except) {
			if (key === hunkGroupToKey(except)) continue;
		}

		const assignments = value.get(change.path);
		if (!assignments) continue;
		headers.push(...assignments);
	}

	return headers;
}

export function filterChangesByGroup(
	changes: TreeChange[],
	group: HunkGroup,
	assignments: HunkAssignments
) {
	const stackGroup = assignments.get(hunkGroupToKey(group));

	if (!stackGroup) return [];

	const filteredChanges = [];
	for (const change of changes) {
		const pathGroup = stackGroup.get(change.path);
		if (pathGroup) {
			filteredChanges.push(change);
		}
	}

	return filteredChanges;
}
