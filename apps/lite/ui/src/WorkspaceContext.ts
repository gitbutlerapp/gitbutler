import type { BranchOperand, CommitOperand, HunkOperand, Operand } from "#ui/operands.ts";
import type { OperationType } from "#ui/operations/operation.ts";
import type { OutlineMode, TransferMode } from "#ui/outline/mode.ts";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";
import type { Workspace } from "#ui/workspace.ts";
import type { AbsorptionTarget, RefInfo } from "@gitbutler/but-sdk";
import { createContext } from "react";

export type OutlineSelectionContext = {
	outlineSelection: Operand | null;
};

export const OutlineSelectionContext = createContext({} as OutlineSelectionContext);
OutlineSelectionContext.displayName = "OutlineSelectionContext";

export type OutlineSelectionActionsContext = {
	selectOutline: (projectId: string, selection: Operand | null) => void;
	updateRewrittenCommitReferences: (
		projectId: string,
		replacedCommits: Record<string, string>,
		headInfo: RefInfo,
	) => void;
	updateRewrittenBranchReferences: (
		projectId: string,
		oldBranch: BranchOperand,
		newBranch: BranchOperand,
	) => void;
};

export const OutlineSelectionActionsContext = createContext({} as OutlineSelectionActionsContext);
OutlineSelectionActionsContext.displayName = "OutlineSelectionActionsContext";

export type FilesSelectionContext = {
	filesSelection: string | null;
	selectFiles: (projectId: string, selection: string | null) => void;
};

export const FilesSelectionContext = createContext({} as FilesSelectionContext);
FilesSelectionContext.displayName = "FilesSelectionContext";

export type DiffSelectionContext = {
	diffSelection: HunkOperand | null;
	selectDiff: (projectId: string, selection: HunkOperand | null) => void;
};

export const DiffSelectionContext = createContext({} as DiffSelectionContext);
DiffSelectionContext.displayName = "DiffSelectionContext";

export type OutlineModeContext = {
	outlineMode: OutlineMode;
	startRewordCommit: (projectId: string, commit: CommitOperand) => void;
	startRenameBranch: (projectId: string, branch: BranchOperand) => void;
	enterTransferMode: (projectId: string, mode: TransferMode) => void;
	enterKeyboardTransferMode: (
		projectId: string,
		source: Operand,
		operationType?: OperationType,
	) => void;
	enterAbsorbMode: (projectId: string, source: Operand, sourceTarget: AbsorptionTarget) => void;
	updatePointerTransfer: (
		projectId: string,
		target: Operand | null,
		operationType: OperationType | null,
	) => void;
	updateTransferOperationType: (projectId: string, operationType: OperationType) => void;
	exitMode: (projectId: string) => void;
	cancelMode: (projectId: string) => void;
};

export const OutlineModeContext = createContext({} as OutlineModeContext);
OutlineModeContext.displayName = "OutlineModeContext";

export type WorkspaceRegistryContext = ProjectRegistry<Workspace>;

export const WorkspaceRegistryContext = createContext({} as WorkspaceRegistryContext);
WorkspaceRegistryContext.displayName = "WorkspaceRegistryContext";
