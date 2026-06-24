import {
	type BranchOperand,
	type CommitOperand,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { type TransferOperationMode } from "#ui/outline/mode.ts";
import * as workspace from "#ui/projects/workspace/state.ts";
import type { RootState } from "#ui/store.ts";
import { type AbsorptionTarget, type RefInfo, type RelativeTo } from "@gitbutler/but-sdk";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "ProjectPicker" };

export type DiffStyle = "split" | "unified";

type ProjectState = {
	detailsFullWindow: boolean;
	dialog: Dialog;
	filesVisible: boolean;
	preferredDiffStyle: DiffStyle;
	workspace: workspace.WorkspaceState;
};

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

const createInitialProjectState = (): ProjectState => ({
	detailsFullWindow: false,
	dialog: { _tag: "None" },
	filesVisible: true,
	preferredDiffStyle: "split",
	workspace: workspace.createInitialState(),
});

const initialProjectState: ProjectState = createInitialProjectState();

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

const projectSlice = createSlice({
	name: "project",
	initialState,
	reducers: {
		selectOutline: (
			state,
			action: PayloadAction<{ projectId: string; selection: Operand | null }>,
		) => {
			const { projectId, selection } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectOutline(projectState.workspace, selection);
		},
		selectFiles: (
			state,
			action: PayloadAction<{ projectId: string; selection: string | null }>,
		) => {
			const { projectId, selection } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectFiles(projectState.workspace, selection);
		},
		selectDiff: (
			state,
			action: PayloadAction<{ projectId: string; selection: HunkOperand | null }>,
		) => {
			const { projectId, selection } = action.payload;
			const projectState = ensureProjectState(state, projectId);
			workspace.selectDiff(projectState.workspace, selection);
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
		setCommitChecked: (
			state,
			action: PayloadAction<{ projectId: string; commitId: string; checked: boolean }>,
		) => {
			const { projectId, commitId, checked } = action.payload;
			workspace.setCommitChecked(ensureProjectState(state, projectId).workspace, commitId, checked);
		},
		setCommitsChecked: (
			state,
			action: PayloadAction<{ projectId: string; commitIds: Array<string>; checked: boolean }>,
		) => {
			const { projectId, commitIds, checked } = action.payload;
			workspace.setCommitsChecked(
				ensureProjectState(state, projectId).workspace,
				commitIds,
				checked,
			);
		},
		clearCheckedCommits: (state, action: PayloadAction<{ projectId: string }>) => {
			workspace.clearCheckedCommits(ensureProjectState(state, action.payload.projectId).workspace);
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
		setPreferredDiffStyle: (
			state,
			action: PayloadAction<{ projectId: string; diffStyle: DiffStyle }>,
		) => {
			const { projectId, diffStyle } = action.payload;
			ensureProjectState(state, projectId).preferredDiffStyle = diffStyle;
		},
		togglePreferredDiffStyle: (state, action: PayloadAction<{ projectId: string }>) => {
			const projectState = ensureProjectState(state, action.payload.projectId);
			projectState.preferredDiffStyle =
				projectState.preferredDiffStyle === "split" ? "unified" : "split";
		},
		setDetailsFullWindow: (
			state,
			action: PayloadAction<{ projectId: string; fullWindow: boolean }>,
		) => {
			const { projectId, fullWindow } = action.payload;
			ensureProjectState(state, projectId).detailsFullWindow = fullWindow;
		},
		toggleDetailsFullWindow: (state, action: PayloadAction<{ projectId: string }>) => {
			const projectState = ensureProjectState(state, action.payload.projectId);
			projectState.detailsFullWindow = !projectState.detailsFullWindow;
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
		openProjectPicker: (state, action: PayloadAction<{ projectId: string }>) => {
			ensureProjectState(state, action.payload.projectId).dialog = {
				_tag: "ProjectPicker",
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

export const selectProjectPreferredDiffStyle = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).preferredDiffStyle;

export const selectProjectDetailsFullWindow = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).detailsFullWindow;

export const selectProjectDialogState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).dialog;

const selectProjectWorkspaceState = (state: RootState, projectId: string) =>
	selectProjectState(state, projectId).workspace;

export const selectProjectSelectionOutline = (state: RootState, projectId: string) =>
	workspace.selectSelectionOutlineState(selectProjectWorkspaceState(state, projectId));

export const selectProjectSelectionFiles = (state: RootState, projectId: string) =>
	workspace.selectSelectionFilesState(selectProjectWorkspaceState(state, projectId));

export const selectProjectSelectionDiff = (state: RootState, projectId: string) =>
	workspace.selectSelectionDiffState(selectProjectWorkspaceState(state, projectId));

export const selectProjectOutlineModeState = (state: RootState, projectId: string) =>
	workspace.selectMode(selectProjectWorkspaceState(state, projectId));

export const selectProjectHighlightedCommitIds = (state: RootState, projectId: string) =>
	workspace.selectHighlightedCommitIds(selectProjectWorkspaceState(state, projectId));

export const selectProjectCommitChecked = (state: RootState, projectId: string, commitId: string) =>
	workspace.selectCommitChecked(selectProjectWorkspaceState(state, projectId), commitId);

export const selectProjectCheckedCommitCount = (state: RootState, projectId: string) =>
	workspace.selectCheckedCommitCount(selectProjectWorkspaceState(state, projectId));

export const selectProjectHasCheckedCommits = (state: RootState, projectId: string) =>
	workspace.selectHasCheckedCommits(selectProjectWorkspaceState(state, projectId));

export const selectProjectCommitTarget = (state: RootState, projectId: string) =>
	workspace.selectCommitTarget(selectProjectWorkspaceState(state, projectId));
