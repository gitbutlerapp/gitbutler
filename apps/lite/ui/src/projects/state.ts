import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import type { RootState } from "#ui/store.ts";
import { type AbsorptionTarget, type RefInfo, type RelativeTo } from "@gitbutler/but-sdk";
import { type BranchOperand, type CommitOperand, type Operand } from "#ui/operands.ts";
import * as workspace from "#ui/projects/workspace/state.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { type TransferOperationMode } from "#ui/outline/mode.ts";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" };

type ProjectState = {
	dialog: Dialog;
	filesVisible: boolean;
	workspace: workspace.WorkspaceState;
};

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

const initialProjectState: ProjectState = {
	dialog: { _tag: "None" },
	filesVisible: true,
	workspace: workspace.initialState,
};

const initialState: ProjectSliceState = {
	byProjectId: {},
};

const createProjectState = (): ProjectState => ({
	dialog: { _tag: "None" },
	filesVisible: true,
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
		updateRewrittenBranchReferences: (
			state,
			action: PayloadAction<{
				projectId: string;
				oldBranch: BranchOperand;
				newBranch: BranchOperand;
			}>,
		) => {
			const { projectId, oldBranch, newBranch } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.updateRewrittenBranchReferences(projectState.workspace, oldBranch, newBranch);
		},
		enterTransferMode: (
			state,
			action: PayloadAction<{ projectId: string; mode: TransferOperationMode }>,
		) => {
			const { projectId, mode } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterTransferMode(projectState.workspace, mode);
		},
		enterAbsorbMode: (
			state,
			action: PayloadAction<{
				projectId: string;
				source: Operand;
				sourceTarget: AbsorptionTarget;
			}>,
		) => {
			const { projectId, source, sourceTarget } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.enterAbsorbMode(projectState.workspace, source, sourceTarget);
		},
		updatePointerTransfer: (
			state,
			action: PayloadAction<{
				projectId: string;
				target: Operand | null;
				operationType: OperationType | null;
			}>,
		) => {
			const { projectId, target, operationType } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.updatePointerTransfer(projectState.workspace, target, operationType);
		},
		updateTransferOperationType: (
			state,
			action: PayloadAction<{
				projectId: string;
				operationType: OperationType;
			}>,
		) => {
			const { projectId, operationType } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.updateTransferOperationType(projectState.workspace, operationType);
		},
		exitMode: (state, action: PayloadAction<{ projectId: string }>) => {
			workspace.exitMode(ensureProjectState(state, action.payload.projectId).workspace);
		},
		cancelMode: (state, action: PayloadAction<{ projectId: string }>) => {
			workspace.cancelMode(ensureProjectState(state, action.payload.projectId).workspace);
		},
		setHighlightedCommitIds: (
			state,
			action: PayloadAction<{ projectId: string; commitIds: Array<string> | null }>,
		) => {
			const { projectId, commitIds } = action.payload;
			workspace.setHighlightedCommitIds(ensureProjectState(state, projectId).workspace, commitIds);
		},
		setCommitTarget: (
			state,
			action: PayloadAction<{ projectId: string; commitTarget: RelativeTo | null }>,
		) => {
			const { projectId, commitTarget } = action.payload;
			workspace.setCommitTarget(ensureProjectState(state, projectId).workspace, commitTarget);
		},
		updateRewrittenCommitReferences: (
			state,
			action: PayloadAction<{
				projectId: string;
				replacedCommits: Record<string, string>;
				headInfo: RefInfo;
			}>,
		) => {
			const { projectId, replacedCommits, headInfo } = action.payload;
			workspace.updateRewrittenCommitReferences(
				ensureProjectState(state, projectId).workspace,
				replacedCommits,
				headInfo,
			);
		},
		toggleFiles: (state, action: PayloadAction<{ projectId: string }>) => {
			const projectState = ensureProjectState(state, action.payload.projectId);
			projectState.filesVisible = !projectState.filesVisible;
		},
		openCommandPalette: (
			state,
			action: PayloadAction<{
				projectId: string;
			}>,
		) => {
			const { projectId } = action.payload;
			ensureProjectState(state, projectId).dialog = {
				_tag: "CommandPalette",
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

export const selectProjectFilesVisible = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).filesVisible;

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

export const selectProjectHighlightedCommitIds = (state: RootState, projectId: string) =>
	workspace.selectHighlightedCommitIds(selectProjectWorkspaceState(state, projectId));

export const selectProjectCommitTarget = (state: RootState, projectId: string) =>
	workspace.selectCommitTarget(selectProjectWorkspaceState(state, projectId));
