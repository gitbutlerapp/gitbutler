import { createSelectByPrefix, createSelectNotIn } from '$lib/state/customSelectors';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	createEntityAdapter,
	createSelector,
	createSlice,
	type EntityState,
	type PayloadAction
} from '@reduxjs/toolkit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment } from '$lib/hunks/hunk';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

function sortLikeLs(a: string, b: string) {
	return a.localeCompare(b, undefined, {
		numeric: true,
		sensitivity: 'base'
	});
}

const changeAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => sortLikeLs(a.path, b.path)
});

const assignmentAdapter = createEntityAdapter<HunkAssignment, string>({
	selectId: (c) => `${c.stackId}-${c.path}-${c.hunkHeader?.newStart}`,
	sortComparer: (a, b) => sortLikeLs(a.path, b.path)
});

export type CheckmarkSelection = {
	checkmarkId: string;
	stackId: string | null;
	path: string;
	assignmentId: string;
	changeId: string;
	lines: LineId[];
};

function checkmarkKey(args: { stackId: string | null; path: string; hunkHeader: string | null }) {
	return `${args.stackId}-${args.path}-${args.hunkHeader}`;
}

const checkmarkAdapter = createEntityAdapter<CheckmarkSelection, string>({
	selectId: (c) => c.checkmarkId,
	sortComparer: (a, b) => sortLikeLs(a.checkmarkId, b.checkmarkId)
});

export const selectAssignments = createSelector([selectSelf], (rootState) => rootState.assignments);
export const selectSelections = createSelector([selectSelf], (rootState) => rootState.selections);

const selectCheckmarksByStackId = createSelector(
	[selectSelections, (_: AssignmentState, stackId: string | null) => stackId],
	(selections, stackId) =>
		selections.ids
			.map((id) => selections.entities[id]!)
			.filter((c) => c.checkmarkId.startsWith(`${stackId}`))
);

export const assignmentSelectors = {
	...assignmentAdapter.getSelectors(),
	selectByPrefix: createSelectByPrefix<HunkAssignment>(),
	selectNotIn: createSelectNotIn<HunkAssignment>()
};

export const checkboxSelectors = {
	...checkmarkAdapter.getSelectors(),
	selectByPrefix: createSelectByPrefix<CheckmarkSelection>(),
	selectNotIn: createSelectNotIn<CheckmarkSelection>()
};

function selectionKey(assignment: HunkAssignment) {
	return `${assignment.stackId}-${assignment.path}-${assignment.hunkHeader?.newStart}`;
}

export const assignmentSlice = createSlice({
	name: 'assignments',
	initialState: {
		changes: changeAdapter.getInitialState(),
		assignments: assignmentAdapter.getInitialState(),
		selections: checkmarkAdapter.getInitialState()
	},
	reducers: {
		clearCheckmarks(state, action: PayloadAction<{ stackId: string | null }>) {
			const items = selectCheckmarksByStackId(state, action.payload.stackId);
			state.selections = checkmarkAdapter.removeMany(
				state.selections,
				items.map((s) => checkmarkAdapter.selectId(s))
			);
		},
		updateAssignments(
			state,
			action: PayloadAction<{ assignments: HunkAssignment[]; changes: TreeChange[] }>
		) {
			state.changes = changeAdapter.addMany(
				changeAdapter.getInitialState(),
				action.payload.changes
			);
			const removals = assignmentSelectors.selectNotIn(
				state.assignments,
				action.payload.assignments.map((a) => assignmentAdapter.selectId(a))
			);
			const removedKeys = removals.map((r) => selectionKey(r));
			if (removedKeys.length > 0) {
				state.assignments = assignmentAdapter.removeMany(state.assignments, removedKeys);
			}
			state.assignments = assignmentAdapter.upsertMany(
				assignmentAdapter.getInitialState(),
				action.payload.assignments
			);
			return state;
		},
		checkLine(
			state,
			action: PayloadAction<{
				stackId: string | null;
				path: string;
				hunkHeader: string;
				line: LineId;
			}>
		) {
			const key = checkmarkKey(action.payload);
			const selection = checkboxSelectors.selectById(state.selections, key);
			const { stackId, line } = action.payload;
			if (selection) {
				state.selections = checkmarkAdapter.upsertOne(state.selections, {
					...selection,
					lines: [...selection.lines, line]
				});
			} else {
				const assignment = assignmentSelectors.selectById(state.assignments, key);
				if (!assignment) {
					throw new Error(`Expected to find assignment: ${key} `);
				}
				state.selections = checkmarkAdapter.addOne(state.selections, {
					checkmarkId: key,
					stackId: stackId,
					path: assignment.path,
					assignmentId: key,
					changeId: `${stackId}-${assignment.path}`,
					lines: [line]
				});
			}
		},
		uncheckLine(
			state,
			action: PayloadAction<{ stackId: string | null; path: string; header: string; line: LineId }>
		) {
			const { stackId, path, header, line } = action.payload;
			const key = `${stackId}-${path}-${header}`;
			const selection = checkboxSelectors.selectById(state.selections, key);
			if (selection) {
				const assignment = assignmentSelectors.selectById(
					state.assignments,
					selection.assignmentId
				);
				if (!assignment) {
					throw new Error(`Expected to find assignment: ${key} `);
				}
				if (assignment.hunkHeader === null) {
					// add all other lines
				} else {
					if (selection.lines.length === 0) {
						state.selections = checkmarkAdapter.upsertOne(state.selections, {
							...selection,
							lines: [line]
						});
					} else {
						state.selections = checkmarkAdapter.upsertOne(state.selections, {
							...selection,
							lines: selection.lines.filter(
								(l) => l.newLine !== line.newLine || l.oldLine !== line.oldLine
							)
						});
					}
				}
			}
		},
		checkHunk(
			state,
			action: PayloadAction<{ stackId: string | null; path: string; hunkHeader: string | null }>
		) {
			const { stackId, path, hunkHeader: header } = action.payload;
			const key = checkmarkKey(action.payload);
			const assignment = assignmentSelectors.selectById(
				state.assignments,
				`${stackId}-${path}-${header}`
			);
			if (assignment) {
				state.selections = checkmarkAdapter.upsertOne(state.selections, {
					checkmarkId: key,
					stackId: stackId,
					path: assignment.path,
					assignmentId: key,
					changeId: `${stackId}-${assignment.path}`,
					lines: []
				});
			}
		},
		uncheckHunk(
			state,
			action: PayloadAction<{ stackId: string | null; path: string; hunkHeader: string | null }>
		) {
			const key = checkmarkKey(action.payload);
			state.selections = checkmarkAdapter.removeOne(state.selections, key);
		},
		checkFile(state, action: PayloadAction<{ stackId: string | null; path: string }>) {
			const { stackId, path } = action.payload;
			const prefix = `${stackId}-${path}-`;
			const assignments = assignmentSelectors.selectByPrefix(state.assignments, prefix);

			for (const assignment of assignments) {
				const key = assignmentAdapter.selectId(assignment);
				state.selections = checkmarkAdapter.upsertOne(state.selections, {
					checkmarkId: key,
					stackId: stackId,
					path: assignment.path,
					assignmentId: key,
					changeId: `${stackId}-${assignment.path}`,
					lines: []
				});
			}
		},
		uncheckFile(state, action: PayloadAction<{ stackId: string | null; path: string }>) {
			const { stackId, path } = action.payload;
			const prefix = `${stackId}-${path}-`;
			const selections = checkboxSelectors.selectByPrefix(state.selections, prefix);
			state.selections = checkmarkAdapter.removeMany(
				state.selections,
				selections.map((a) => a.assignmentId)
			);
		},
		checkStack(state, action: PayloadAction<{ stackId: string | null }>) {
			const { stackId } = action.payload;
			const prefix = `${stackId}-`;
			const assignments = assignmentSelectors.selectByPrefix(state.assignments, prefix);

			for (const assignment of assignments) {
				const key = assignmentAdapter.selectId(assignment);
				state.selections = checkmarkAdapter.upsertOne(state.selections, {
					checkmarkId: key,
					stackId: stackId,
					path: assignment.path,
					assignmentId: key,
					changeId: `${stackId}-${assignment.path}`,
					lines: []
				});
			}
		},
		uncheckStack(state, action: PayloadAction<{ stackId: string | null }>) {
			const { stackId } = action.payload;
			const prefix = `${stackId}-`;
			const selections = checkboxSelectors.selectByPrefix(state.selections, prefix);
			state.selections = checkmarkAdapter.removeMany(
				state.selections,
				selections.map((s) => s.checkmarkId)
			);
		}
	}
});

type AssignmentState = ReturnType<typeof assignmentSlice.getInitialState>;

function selectSelf(state: ReturnType<typeof assignmentSlice.getInitialState>) {
	return state;
}

export const selectChanges = createSelector([selectSelf], (rootState) => rootState.changes);

const selectChangeByPath = createSelector(
	[selectSelf, (_, path: string) => path],
	(rootState, path: string) => rootState.changes.entities[path]
);

const selectChangesByStackId = createSelector(
	[selectChanges, selectAssignments, (_: AssignmentState, stackId: string | null) => stackId],
	(changes, assignments, stackId) => {
		const paths = new Set(
			Object.values(assignments.entities)
				.filter((a) => a.stackId === stackId)
				.filter(isDefined)
				.map((a) => a.path)
		);
		return changes.ids.map((id) => changes.entities[id]!).filter((c) => paths.has(c.path));
	}
);

const selectedChangesByStackId = createSelector(
	[
		selectSelections,
		selectChangesByStackId,
		(_: AssignmentState, stackId: string | null) => stackId
	],
	(selections, changes, stackId) =>
		changes.filter((change) => `${stackId}-${change.path}` in selections)
);

export const changeSelectors = {
	...changeAdapter.getSelectors(),
	selectChangeByPath,
	selectChangesByStackId,
	selectedChangesByStackId
};

export const assignmentsByPrefix = createSelector(
	[
		(assignments: EntityState<HunkAssignment, string>) => assignments,
		(_, prefix: string) => prefix
	],
	(assignments, prefix: string) => assignmentSelectors.selectByPrefix(assignments, prefix)
);

export type CheckStatus = 'checked' | 'indeterminate' | 'unchecked';

export const hunkCheckStatus = createSelector(
	[
		selectSelections,
		(_, hunkId: { stackId: string | null; path: string; header: string }) => {
			return hunkId;
		}
	],
	(selections, { stackId, path, header }) => {
		const selection = checkboxSelectors.selectById(selections, `${stackId}-${path}-${header}`);
		if (!selection) {
			return { selected: false, lines: [] };
		} else {
			return { selected: true, lines: selection.lines };
		}
	}
);

export const fileCheckStatus = createSelector(
	[
		selectSelections,
		selectAssignments,
		(_, args: { stackId: string | null; path: string }) => {
			return args;
		}
	],
	(selections, assignments, { stackId, path }) => {
		const selection = checkboxSelectors.selectByPrefix(selections, `${stackId}-${path}-`);
		const prefix = `${stackId}-${path}-`;
		const stackAssignments = assignmentSelectors.selectByPrefix(assignments, prefix);
		if (!selection || selection.length === 0) {
			return 'unchecked';
		} else if (selection.length === stackAssignments.length) {
			return 'checked';
		} else {
			return 'indeterminate';
		}
	}
);

export const folderCheckStatus = createSelector(
	[
		selectSelections,
		selectAssignments,
		(_, args: { stackId: string | null; path: string }) => {
			return args;
		}
	],
	(selections, assignments, { stackId, path }) => {
		// TOOD: What path sepearator do we use on Windows?
		const keyPrefix = `${stackId}-${path}`;
		const matches = assignmentSelectors.selectByPrefix(assignments, keyPrefix);
		if (matches.length === 0) {
			return 'unchecked';
		} else if (
			matches.every(
				(a) => `${a.stackId}-${a.path}-${a.hunkHeader?.newStart.toString()}` in selections.entities
			)
		) {
			return 'checked';
		} else if (
			matches.some(
				(a) => `${a.stackId}-${a.path}-${a.hunkHeader?.newStart.toString()}` in selections.entities
			)
		) {
			return 'indeterminate';
		}
		return 'unchecked';
	}
);

export const assignmentActions = assignmentSlice.actions;

export const assignmentReducer = assignmentSlice.reducer;
