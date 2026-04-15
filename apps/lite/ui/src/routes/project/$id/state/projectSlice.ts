import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import type { RootState } from "#ui/state/store.ts";
import { type BranchItem, type CommitItem, type Item } from "../workspace/Item.ts";
import type { OperationSource } from "../workspace/OperationSource.ts";
import * as layout from "./layout.ts";
import * as workspace from "./workspace.ts";

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
		selectItem: (state, action: PayloadAction<{ projectId: string; item: Item | null }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.selectItem(projectState.workspace, item);
		},
		selectHunk: (state, action: PayloadAction<{ projectId: string; hunk: string | null }>) => {
			const { projectId, hunk } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPreview(projectState.layout);
			workspace.selectHunk(projectState.workspace, hunk);
		},
		startRewordCommit: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.startRewordCommit(projectState.workspace, item);
		},
		startRenameBranch: (state, action: PayloadAction<{ projectId: string; item: BranchItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.startRenameBranch(projectState.workspace, item);
		},
		openCommitFiles: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.openCommitFiles(projectState.workspace, item);
		},
		closeCommitFiles: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.closeCommitFiles(projectState.workspace, item);
		},
		toggleCommitFiles: (state, action: PayloadAction<{ projectId: string; item: CommitItem }>) => {
			const { projectId, item } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.toggleCommitFiles(projectState.workspace, item);
		},
		enterRubMode: (
			state,
			action: PayloadAction<{ projectId: string; source: OperationSource }>,
		) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.enterRubMode(projectState.workspace, source);
		},
		enterMoveMode: (
			state,
			action: PayloadAction<{ projectId: string; source: OperationSource }>,
		) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			layout.focusPrimary(projectState.layout);
			workspace.enterMoveMode(projectState.workspace, source);
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
		focusPrimary: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.focusPrimary(ensureProjectState(state, action.payload.projectId).layout);
		},
		focusPreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.focusPreview(ensureProjectState(state, action.payload.projectId).layout);
		},
		closePreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.closePreview(ensureProjectState(state, action.payload.projectId).layout);
		},
		togglePreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.togglePreview(ensureProjectState(state, action.payload.projectId).layout);
		},
		openFullscreenPreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.openFullscreenPreview(ensureProjectState(state, action.payload.projectId).layout);
		},
		closeFullscreenPreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.closeFullscreenPreview(ensureProjectState(state, action.payload.projectId).layout);
		},
		toggleFullscreenPreview: (state, action: PayloadAction<{ projectId: string }>) => {
			layout.toggleFullscreenPreview(ensureProjectState(state, action.payload.projectId).layout);
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

export const selectProjectSelectedHunk = (state: RootState, projectId: string) =>
	workspace.selectSelectedHunk(selectProjectWorkspaceState(state, projectId));

export const selectProjectWorkspaceModeState = (state: RootState, projectId: string) =>
	workspace.selectMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectExpandedCommitId = (state: RootState, projectId: string) =>
	workspace.selectExpandedCommitId(selectProjectWorkspaceState(state, projectId));

export const selectProjectHighlightedCommitIds = (state: RootState, projectId: string) =>
	workspace.selectHighlightedCommitIds(selectProjectWorkspaceState(state, projectId));
