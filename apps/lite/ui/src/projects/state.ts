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
		setDetailsSelectionScope: withProject(projectReducers.setDetailsSelectionScope),
		selectUncommittedFiles: withProject(projectReducers.selectUncommittedFiles),
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
		setHighlightedChangeIds: withProject(projectReducers.setHighlightedChangeIds),
		checkCommit: withProject(projectReducers.checkCommit),
		checkCommits: withProject(projectReducers.checkCommits),
		setCheckedCommits: withProject(projectReducers.setCheckedCommits),
		clearCheckedCommits: withProject(projectReducers.clearCheckedCommits),
		setCommitTarget: withProject(projectReducers.setCommitTarget),
		toggleFiles: withProject(projectReducers.toggleFiles),
		setDetailsFullWindow: withProject(projectReducers.setDetailsFullWindow),
		toggleDetailsFullWindow: withProject(projectReducers.toggleDetailsFullWindow),
		openDialog: withProject(projectReducers.openDialog),
		closeDialog: withProject(projectReducers.closeDialog),
	},
	selectors: {
		selectFilesVisible: fromProject(projectSelectors.selectFilesVisible),
		selectCanShowFiles: fromProject(projectSelectors.selectCanShowFiles),
		selectDetailsFullWindow: fromProject(projectSelectors.selectDetailsFullWindow),
		selectDetailsSelectionScope: fromProject(projectSelectors.selectDetailsSelectionScope),
		selectDialogState: fromProject(projectSelectors.selectDialogState),
		selectSelectionUncommittedFiles: fromProject(projectSelectors.selectSelectionUncommittedFiles),
		selectIsSelectedOutline: fromProject(projectSelectors.selectIsSelectedOutline),
		selectSelectionOutline: fromProject(projectSelectors.selectSelectionOutline),
		selectSelectionFiles: fromProject(projectSelectors.selectSelectionFiles),
		selectSelectionDiff: fromProject(projectSelectors.selectSelectionDiff),
		selectOutlineModeState: fromProject(projectSelectors.selectOutlineModeState),
		selectHighlightedChangeIds: fromProject(projectSelectors.selectHighlightedChangeIds),
		selectCommitChecked: fromProject(projectSelectors.selectCommitChecked),
		selectCheckedCommits: fromProject(projectSelectors.selectCheckedCommits),
		selectCheckedCommitOperands: fromProject(projectSelectors.selectCheckedCommitOperands),
		selectCheckedCommitCount: fromProject(projectSelectors.selectCheckedCommitCount),
		selectHasCheckedCommits: fromProject(projectSelectors.selectHasCheckedCommits),
		selectCommitTarget: fromProject(projectSelectors.selectCommitTarget),
	},
});
