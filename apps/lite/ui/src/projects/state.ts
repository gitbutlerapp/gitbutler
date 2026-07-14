import {
	createInitialProjectState,
	projectReducers,
	projectSelectors,
	type ProjectState,
} from "#ui/projects/project.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

const initialState: ProjectSliceState = {
	byProjectId: {},
};

const ensureProjectState = (state: ProjectSliceState, projectId: string): ProjectState => {
	const existingState = state.byProjectId[projectId];
	if (existingState) return existingState;

	const projectState = createInitialProjectState();
	state.byProjectId[projectId] = projectState;
	return projectState;
};

const initialProjectState: ProjectState = createInitialProjectState();
const selectProjectState = (state: ProjectSliceState, projectId: string): ProjectState =>
	state.byProjectId[projectId] ?? initialProjectState;

const withProject =
	<T>(reducer: (state: ProjectState, payload: T) => void) =>
	(state: ProjectSliceState, action: PayloadAction<T & { projectId: string }>) => {
		reducer(ensureProjectState(state, action.payload.projectId), action.payload);
	};

const fromProject =
	<T extends Array<unknown>, R>(selector: (state: ProjectState, ...args: T) => R) =>
	(state: ProjectSliceState, projectId: string, ...args: T): R =>
		selector(selectProjectState(state, projectId), ...args);

export const projectSlice = createSlice({
	name: "project",
	initialState,
	reducers: {
		selectOutline: withProject(projectReducers.selectOutline),
		selectFiles: withProject(projectReducers.selectFiles),
		selectDiff: withProject(projectReducers.selectDiff),
		startRewordCommit: withProject(projectReducers.startRewordCommit),
		startRenameBranch: withProject(projectReducers.startRenameBranch),
		updateRewrittenBranchReferences: withProject(projectReducers.updateRewrittenBranchReferences),
		enterTransferMode: withProject(projectReducers.enterTransferMode),
		enterKeyboardTransferMode: withProject(projectReducers.enterKeyboardTransferMode),
		enterAbsorbMode: withProject(projectReducers.enterAbsorbMode),
		updatePointerTransfer: withProject(projectReducers.updatePointerTransfer),
		updateTransferOperationType: withProject(projectReducers.updateTransferOperationType),
		exitMode: withProject(projectReducers.exitMode),
		cancelMode: withProject(projectReducers.cancelMode),
		setCommitTarget: withProject(projectReducers.setCommitTarget),
		updateRewrittenCommitReferences: withProject(projectReducers.updateRewrittenCommitReferences),
	},
	selectors: {
		selectIsSelectedOutline: fromProject(projectSelectors.selectIsSelectedOutline),
		selectSelectionOutline: fromProject(projectSelectors.selectSelectionOutline),
		selectSelectionFiles: fromProject(projectSelectors.selectSelectionFiles),
		selectSelectionDiff: fromProject(projectSelectors.selectSelectionDiff),
		selectOutlineModeState: fromProject(projectSelectors.selectOutlineModeState),
		selectCommitTarget: fromProject(projectSelectors.selectCommitTarget),
	},
});
