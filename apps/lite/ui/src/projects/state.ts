import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import type { RootState } from "#ui/store.ts";
import { type BranchOperand, type CommitOperand, type Operand } from "#ui/operands.ts";
import { type Panel } from "#ui/panels.ts";
import * as panels from "#ui/panels/state.ts";
import * as workspace from "#ui/projects/workspace/state.ts";
import { OperationType } from "#ui/operations/operation.ts";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette"; focusedPanel: Panel | null };

type ProjectState = {
	dialog: Dialog;
	panels: panels.PanelsState;
	workspace: workspace.WorkspaceState;
};

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

const initialProjectState: ProjectState = {
	dialog: { _tag: "None" },
	panels: panels.initialState,
	workspace: workspace.initialState,
};

const initialState: ProjectSliceState = {
	byProjectId: {},
};

const createProjectState = (): ProjectState => ({
	dialog: { _tag: "None" },
	panels: panels.createInitialState(),
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
		selectOutline: (state, action: PayloadAction<{ projectId: string; selection: Operand }>) => {
			const { projectId, selection } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectOutline(projectState.workspace, selection);
		},
		selectFiles: (state, action: PayloadAction<{ projectId: string; selection: Operand }>) => {
			const { projectId, selection } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectFiles(projectState.workspace, selection);
		},
		startRewordCommit: (
			state,
			action: PayloadAction<{ projectId: string; commit: CommitOperand }>,
		) => {
			const { projectId, commit } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.startRewordCommit(projectState.workspace, commit);
		},
		startRenameBranch: (
			state,
			action: PayloadAction<{ projectId: string; branch: BranchOperand }>,
		) => {
			const { projectId, branch } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.startRenameBranch(projectState.workspace, branch);
		},
		enterRubMode: (state, action: PayloadAction<{ projectId: string; source: Operand }>) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterRubMode(projectState.workspace, source);
		},
		enterCutMode: (state, action: PayloadAction<{ projectId: string; source: Operand }>) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterCutMode(projectState.workspace, source);
		},
		enterMoveMode: (state, action: PayloadAction<{ projectId: string; source: Operand }>) => {
			const { projectId, source } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterMoveMode(projectState.workspace, source);
		},
		enterDragAndDropMode: (
			state,
			action: PayloadAction<{
				projectId: string;
				source: Operand;
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
		setHighlightedCommitIds: (
			state,
			action: PayloadAction<{ projectId: string; commitIds: Array<string> | null }>,
		) => {
			const { projectId, commitIds } = action.payload;
			workspace.setHighlightedCommitIds(ensureProjectState(state, projectId).workspace, commitIds);
		},
		showPanel: (state, action: PayloadAction<{ projectId: string; panel: Panel }>) => {
			panels.showPanel(
				ensureProjectState(state, action.payload.projectId).panels,
				action.payload.panel,
			);
		},
		hidePanel: (state, action: PayloadAction<{ projectId: string; panel: Panel }>) => {
			panels.hidePanel(
				ensureProjectState(state, action.payload.projectId).panels,
				action.payload.panel,
			);
		},
		togglePanel: (state, action: PayloadAction<{ projectId: string; panel: Panel }>) => {
			panels.togglePanel(
				ensureProjectState(state, action.payload.projectId).panels,
				action.payload.panel,
			);
		},
		openCommandPalette: (
			state,
			action: PayloadAction<{
				projectId: string;
				focusedPanel: Panel | null;
			}>,
		) => {
			const { projectId, focusedPanel } = action.payload;
			ensureProjectState(state, projectId).dialog = {
				_tag: "CommandPalette",
				focusedPanel,
			};
		},
		openBranchPicker: (state, action: PayloadAction<{ projectId: string }>) => {
			ensureProjectState(state, action.payload.projectId).dialog = {
				_tag: "BranchPicker",
			};
		},
		openApplyBranchPicker: (state, action: PayloadAction<{ projectId: string }>) => {
			ensureProjectState(state, action.payload.projectId).dialog = {
				_tag: "ApplyBranchPicker",
			};
		},
		closeDialog: (state, action: PayloadAction<{ projectId: string }>) => {
			ensureProjectState(state, action.payload.projectId).dialog = { _tag: "None" };
		},
	},
});

export const projectActions = projectSlice.actions;
export const projectReducer = projectSlice.reducer;

const selectProjectState = (state: RootState, projectId: string): ProjectState =>
	state.project.byProjectId[projectId] ?? initialProjectState;

export const selectProjectPanelsState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).panels;

export const selectProjectDialogState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).dialog;

const selectProjectWorkspaceState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).workspace;

export const selectProjectSelectionOutline = (state: RootState, projectId: string) =>
	workspace.selectSelectionOutlineState(selectProjectWorkspaceState(state, projectId));

export const selectProjectSelectionFiles = (state: RootState, projectId: string) =>
	workspace.selectSelectionFilesState(selectProjectWorkspaceState(state, projectId));

export const selectProjectOutlineModeState = (state: RootState, projectId: string) =>
	workspace.selectMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectOperationModeState = (state: RootState, projectId: string) =>
	workspace.selectOperationMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectHighlightedCommitIds = (state: RootState, projectId: string) =>
	workspace.selectHighlightedCommitIds(selectProjectWorkspaceState(state, projectId));
