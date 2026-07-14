import { rewrittenCommitOperand, rewrittenCommitSelection } from "#ui/commit.ts";
import {
	branchOperand,
	commitOperand,
	hunkOperand,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type CommitOperand,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import type { OperationType } from "#ui/operations/operation.ts";
import {
	absorbOutlineMode,
	defaultOutlineMode,
	isValidOutlineModeForSelection,
	keyboardTransferMode,
	pointerTransferMode,
	renameBranchOutlineMode,
	rewordCommitOutlineMode,
	transferOutlineMode,
	type OutlineMode,
	type TransferMode,
} from "#ui/outline/mode.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { AbsorptionTarget, RefInfo } from "@gitbutler/but-sdk";

export type Selection = {
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

export type Workspace = {
	mode: OutlineMode;
	selection: Selection;
};

const selectOutline = (workspace: Workspace, selection: Operand | null): Workspace => {
	if (
		selection &&
		workspace.selection.outline &&
		operandEquals(workspace.selection.outline, selection)
	)
		return workspace;

	const mode =
		selection && isValidOutlineModeForSelection({ mode: workspace.mode, selection })
			? workspace.mode
			: defaultOutlineMode;
	if (
		workspace.selection.outline === selection &&
		workspace.selection.files === null &&
		workspace.selection.diff === null &&
		workspace.mode === mode
	)
		return workspace;

	return {
		mode,
		selection: { outline: selection, files: null, diff: null },
	};
};

const selectFiles = (workspace: Workspace, selection: string | null): Workspace =>
	workspace.selection.files === selection
		? workspace
		: { ...workspace, selection: { ...workspace.selection, files: selection } };

const selectDiff = (workspace: Workspace, selection: HunkOperand | null): Workspace => {
	const current = workspace.selection.diff;
	if (selection && current && operandEquals(hunkOperand(current), hunkOperand(selection)))
		return workspace;
	if (current === selection) return workspace;

	return { ...workspace, selection: { ...workspace.selection, diff: selection } };
};

const startRewordCommit = (workspace: Workspace, commit: CommitOperand): Workspace => {
	const selection = commitOperand(commit);
	const sameSelection =
		workspace.selection.outline !== null && operandEquals(workspace.selection.outline, selection);

	return {
		mode: rewordCommitOutlineMode({ operand: commit }),
		selection: sameSelection
			? workspace.selection
			: { outline: selection, files: null, diff: null },
	};
};

const startRenameBranch = (workspace: Workspace, branch: BranchOperand): Workspace => {
	const selection = branchOperand(branch);
	const sameSelection =
		workspace.selection.outline !== null && operandEquals(workspace.selection.outline, selection);

	return {
		mode: renameBranchOutlineMode({ operand: branch }),
		selection: sameSelection
			? workspace.selection
			: { outline: selection, files: null, diff: null },
	};
};

const updateRewrittenBranchReferences = (
	workspace: Workspace,
	oldBranch: BranchOperand,
	newBranch: BranchOperand,
): Workspace => {
	const oldBranchOperand = branchOperand(oldBranch);
	const outline =
		workspace.selection.outline?._tag === "Branch" &&
		operandEquals(workspace.selection.outline, oldBranchOperand)
			? branchOperand(newBranch)
			: workspace.selection.outline;
	const mode =
		workspace.mode._tag === "RenameBranch" &&
		operandEquals(branchOperand(workspace.mode.operand), oldBranchOperand)
			? renameBranchOutlineMode({ operand: newBranch })
			: workspace.mode;

	if (outline === workspace.selection.outline && mode === workspace.mode) return workspace;
	return {
		mode,
		selection:
			outline === workspace.selection.outline
				? workspace.selection
				: { ...workspace.selection, outline },
	};
};

const enterTransferMode = (workspace: Workspace, mode: TransferMode): Workspace => ({
	...workspace,
	mode: transferOutlineMode(mode),
});

const enterKeyboardTransferMode = (
	workspace: Workspace,
	source: Operand,
	operationType?: OperationType,
): Workspace => ({
	...workspace,
	mode: transferOutlineMode(
		keyboardTransferMode({
			source,
			operationType: operationType ?? "into",
			restoreSelection: workspace.selection,
		}),
	),
});

const enterAbsorbMode = (
	workspace: Workspace,
	source: Operand,
	sourceTarget: AbsorptionTarget,
): Workspace => ({
	...workspace,
	mode: absorbOutlineMode({
		source,
		restoreSelection: workspace.selection,
		sourceTarget,
	}),
});

const updatePointerTransfer = (
	workspace: Workspace,
	target: Operand | null,
	operationType: OperationType | null,
): Workspace => {
	if (workspace.mode._tag !== "Transfer" || workspace.mode.value._tag !== "Pointer")
		return workspace;

	const mode = workspace.mode.value;
	const sameTarget =
		target === null
			? mode.target === null
			: mode.target !== null && operandEquals(mode.target, target);
	if (sameTarget && mode.operationType === operationType) return workspace;

	return {
		...workspace,
		mode: transferOutlineMode(pointerTransferMode({ source: mode.source, target, operationType })),
	};
};

const updateTransferOperationType = (
	workspace: Workspace,
	operationType: OperationType,
): Workspace => {
	if (workspace.mode._tag !== "Transfer" || workspace.mode.value._tag !== "Keyboard")
		return workspace;
	if (workspace.mode.value.operationType === operationType) return workspace;

	return {
		...workspace,
		mode: transferOutlineMode(
			keyboardTransferMode({
				source: workspace.mode.value.source,
				operationType,
				restoreSelection: workspace.mode.value.restoreSelection,
			}),
		),
	};
};

const exitMode = (workspace: Workspace): Workspace =>
	workspace.mode._tag === "Default" ? workspace : { ...workspace, mode: defaultOutlineMode };

const cancelMode = (workspace: Workspace): Workspace => {
	const restoreSelection =
		workspace.mode._tag === "Absorb"
			? workspace.mode.restoreSelection
			: workspace.mode._tag === "Transfer" && workspace.mode.value._tag === "Keyboard"
				? workspace.mode.value.restoreSelection
				: null;
	if (workspace.mode._tag === "Default") return workspace;

	return {
		mode: defaultOutlineMode,
		selection: restoreSelection ?? workspace.selection,
	};
};

const updateRewrittenCommitReferences = (
	workspace: Workspace,
	replacedCommits: Record<string, string>,
	headInfo: RefInfo,
): Workspace => {
	const outline = rewrittenCommitSelection({
		selection: workspace.selection.outline,
		replacedCommits,
		headInfo,
	});
	const rewrittenCommit =
		workspace.mode._tag === "RewordCommit"
			? rewrittenCommitOperand({
					commit: workspace.mode.operand,
					replacedCommits,
					headInfo,
				})
			: null;
	const mode = rewrittenCommit
		? rewordCommitOutlineMode({ operand: rewrittenCommit })
		: workspace.mode;

	if (outline === workspace.selection.outline && mode === workspace.mode) return workspace;
	return {
		mode,
		selection:
			outline === workspace.selection.outline
				? workspace.selection
				: { ...workspace.selection, outline },
	};
};

export const workspaceTransitions = {
	selectOutline,
	selectFiles,
	selectDiff,
	startRewordCommit,
	startRenameBranch,
	updateRewrittenBranchReferences,
	enterTransferMode,
	enterKeyboardTransferMode,
	enterAbsorbMode,
	updatePointerTransfer,
	updateTransferOperationType,
	exitMode,
	cancelMode,
	updateRewrittenCommitReferences,
};

const resolveNavigationIndexSelection = <T>(
	navigationIndex: NavigationIndex<T>,
	selection: T | null,
	getKey: (item: T) => string,
): T | null =>
	selection !== null && navigationIndexIncludes(navigationIndex, selection, getKey)
		? selection
		: (navigationIndex.items[0] ?? null);

const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

export const resolveOutlineSelection = (
	selection: Operand | null,
	navigationIndex: NavigationIndex<Operand>,
): Operand | null =>
	resolveNavigationIndexSelection(navigationIndex, selection, operandIdentityKey);

export const isOutlineSelected = (
	selection: Operand | null,
	navigationIndex: NavigationIndex<Operand>,
	operand: Operand,
): boolean => {
	const resolved = resolveOutlineSelection(selection, navigationIndex);
	return resolved !== null && operandEquals(resolved, operand);
};

export const resolveFilesSelection = (
	selection: string | null,
	navigationIndex: NavigationIndex<string>,
): string | null => resolveNavigationIndexSelection(navigationIndex, selection, (item) => item);

export const resolveDiffSelection = (
	selection: HunkOperand | null,
	navigationIndex: NavigationIndex<HunkOperand>,
): HunkOperand | null =>
	resolveNavigationIndexSelection(navigationIndex, selection, hunkOperandIdentityKey);
