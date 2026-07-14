import type { BranchOperand, CommitOperand, HunkOperand, Operand } from "#ui/operands.ts";
import type { OperationType } from "#ui/operations/operation.ts";
import type { OutlineMode, TransferMode } from "#ui/outline/mode.ts";
import type { ProjectRegistry } from "#ui/ProjectRegistry.ts";
import type { Workspace } from "#ui/workspace.ts";
import type { AbsorptionTarget, RefInfo } from "@gitbutler/but-sdk";
import { createContext } from "react";

export type OutlineSelectionContext = {
	outlineSelection: Operand | null;
	selectOutline: (selection: Operand | null) => void;
	updateRewrittenCommitReferences: (
		replacedCommits: Record<string, string>,
		headInfo: RefInfo,
	) => void;
	updateRewrittenBranchReferences: (oldBranch: BranchOperand, newBranch: BranchOperand) => void;
};

export const OutlineSelectionContext = createContext({} as OutlineSelectionContext);
OutlineSelectionContext.displayName = "OutlineSelectionContext";

export type FilesSelectionContext = {
	filesSelection: string | null;
	selectFiles: (selection: string | null) => void;
};

export const FilesSelectionContext = createContext({} as FilesSelectionContext);
FilesSelectionContext.displayName = "FilesSelectionContext";

export type DiffSelectionContext = {
	diffSelection: HunkOperand | null;
	selectDiff: (selection: HunkOperand | null) => void;
};

export const DiffSelectionContext = createContext({} as DiffSelectionContext);
DiffSelectionContext.displayName = "DiffSelectionContext";

export type OutlineModeContext = {
	outlineMode: OutlineMode;
	startRewordCommit: (commit: CommitOperand) => void;
	startRenameBranch: (branch: BranchOperand) => void;
	enterTransferMode: (mode: TransferMode) => void;
	enterKeyboardTransferMode: (source: Operand, operationType?: OperationType) => void;
	enterAbsorbMode: (source: Operand, sourceTarget: AbsorptionTarget) => void;
	updatePointerTransfer: (target: Operand | null, operationType: OperationType | null) => void;
	updateTransferOperationType: (operationType: OperationType) => void;
	exitMode: () => void;
	cancelMode: () => void;
};

export const OutlineModeContext = createContext({} as OutlineModeContext);
OutlineModeContext.displayName = "OutlineModeContext";

export type WorkspaceRegistryContext = ProjectRegistry<Workspace>;

export const WorkspaceRegistryContext = createContext({} as WorkspaceRegistryContext);
WorkspaceRegistryContext.displayName = "WorkspaceRegistryContext";
