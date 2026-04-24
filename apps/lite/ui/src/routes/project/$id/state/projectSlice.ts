import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import type { RootState } from "#ui/state/store.ts";
import { type BranchItem, type CommitItem, type Item } from "../workspace/Item.ts";
import * as layout from "./layout.ts";
import * as workspace from "./workspace.ts";
import { OperationType } from "#ui/Operation.ts";

type ProjectState = {
	layout: layout.ProjectLayoutState;
	workspace: workspace.WorkspaceState;
};

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

const initialProjectState: ProjectState = {
	layout: layout.initialState,
	workspace: workspace.initialState,
};

const initialState: ProjectSliceState = {
	byProjectId: {},
};

const createProjectState = (): ProjectState => ({
	layout: layout.createInitialState(),
	workspace: workspace.createInitialState(),
});

const ensureProjectState = (state: ProjectSliceState, projectId: string): ProjectState => {
	const existingState = state.byProjectId[projectId];
	if (existingState) return existingState;

	const projectState = createProjectState();
	state.byProjectId[projectId] = projectState;
	return projectState;
};

const projectSlice = createSlice({
	name: "project",
	initialState,
	reducers: {
		selectItem: (state, action: PayloadAction<{ projectId: string; item: Item }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectItem(projectState.workspace, item);
		},
		startRewordCommit: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.startRewordCommit(projectState.workspace, item);
		},
		startRenameBranch: (state, action: PayloadAction<{ projectId: string; item: BranchItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.startRenameBranch(projectState.workspace, item);
		},
		openCommitFiles: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.openCommitFiles(projectState.workspace, item);
		},
		closeCommitFiles: (state, action: PayloadAction<{ projectId: string }>) => {
			const { projectId } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.closeCommitFiles(projectState.workspace);
		},
		toggleCommitFiles: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.toggleCommitFiles(projectState.workspace, item);
		},
		enterRubMode: (state, action: PayloadAction<{ projectId: string; source: Item }>) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterRubMode(projectState.workspace, source);
		},
		enterMoveMode: (state, action: PayloadAction<{ projectId: string; source: Item }>) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterMoveMode(projectState.workspace, source);
		},
		enterDragAndDropMode: (
			state,
			action: PayloadAction<{
				projectId: string;
				source: Item;
			}>,
		) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterDragAndDropMode(projectState.workspace, source);
		},
		updateDragAndDropMode: (
			state,
			action: PayloadAction<{
				projectId: string;
				operationType: OperationType | null;
			}>,
		) => {
			const { projectId, operationType } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.updateDragAndDropMode(projectState.workspace, operationType);
		},
		exitMode: (state, action: PayloadAction<{ projectId: string }>) => {
			workspace.exitMode(ensureProjectState(state, action.payload.projectId).workspace);
		},
		setExpandedCommitId: (
			state,
			action: PayloadAction<{ projectId: string; commitId: string | null }>,
		) => {
			const { projectId, commitId } = action.payload;
			workspace.setExpandedCommitId(ensureProjectState(state, projectId).workspace, commitId);
		},
		setHighlightedCommitIds: (
			state,
			action: PayloadAction<{ projectId: string; commitIds: Array<string> | null }>,
		) => {
			const { projectId, commitIds } = action.payload;
			workspace.setHighlightedCommitIds(ensureProjectState(state, projectId).workspace, commitIds);
		},
		showPanel: (state, action: PayloadAction<{ projectId: string; panel: layout.Panel }>) => {
			layout.showPanel(
				ensureProjectState(state, action.payload.projectId).layout,
				action.payload.panel,
			);
		},
		hidePanel: (state, action: PayloadAction<{ projectId: string; panel: layout.Panel }>) => {
			layout.hidePanel(
				ensureProjectState(state, action.payload.projectId).layout,
				action.payload.panel,
			);
		},
		togglePanel: (state, action: PayloadAction<{ projectId: string; panel: layout.Panel }>) => {
			layout.togglePanel(
				ensureProjectState(state, action.payload.projectId).layout,
				action.payload.panel,
			);
		},
	},
});

export const projectActions = projectSlice.actions;
export const projectReducer = projectSlice.reducer;

const selectProjectState = (state: RootState, projectId: string): ProjectState =>
	state.project.byProjectId[projectId] ?? initialProjectState;

export const selectProjectLayoutState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).layout;

const selectProjectWorkspaceState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).workspace;

export const selectProjectSelectedItem = (state: RootState, projectId: string) =>
	workspace.selectSelectedItem(selectProjectWorkspaceState(state, projectId));

export const selectProjectWorkspaceModeState = (state: RootState, projectId: string) =>
	workspace.selectMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectOperationModeState = (state: RootState, projectId: string) =>
	workspace.selectOperationMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectExpandedCommitId = (state: RootState, projectId: string) =>
	workspace.selectExpandedCommitId(selectProjectWorkspaceState(state, projectId));

export const selectProjectHighlightedCommitIds = (state: RootState, projectId: string) =>
	workspace.selectHighlightedCommitIds(selectProjectWorkspaceState(state, projectId));
