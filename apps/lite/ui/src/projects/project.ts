import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
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
import type { SelectionScope } from "#ui/selection-scopes.ts";
import { createSelector } from "@reduxjs/toolkit";
import type { AbsorptionTarget, RelativeTo } from "@gitbutler/but-sdk";
import { Match } from "effect";

export type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings" };

export type SelectionState = {
	uncommittedFiles: string | null;
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

type DetailsSelectionScope = Extract<SelectionScope, "uncommitted-files" | "outline">;

type WorkspaceState = {
	checkedChangeIds: Record<string, true>;
	commitTarget: Operand | null;
	detailsSelectionScope: DetailsSelectionScope | null;
	highlightedChangeIds: Array<string>;
	mode: OutlineMode;
	selection: SelectionState;
};

const createInitialSelectionState = (): SelectionState => ({
	uncommittedFiles: null,
	outline: null,
	files: null,
	diff: null,
});

const createInitialWorkspaceState = (): WorkspaceState => ({
	checkedChangeIds: {},
	commitTarget: null,
	detailsSelectionScope: null,
	highlightedChangeIds: [],
	mode: defaultOutlineMode,
	selection: createInitialSelectionState(),
});

export type ProjectState = {
	detailsFullWindow: boolean;
	dialog: Dialog;
	filesVisible: boolean;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	detailsFullWindow: false,
	dialog: { _tag: "None" },
	filesVisible: false,
	workspace: createInitialWorkspaceState(),
});

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

export const projectReducers = {
	setDetailsSelectionScope: (state: ProjectState, { scope }: { scope: DetailsSelectionScope }) => {
		state.workspace.detailsSelectionScope = scope;
	},
	selectUncommittedFiles: (state: ProjectState, { selection }: { selection: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.selection.uncommittedFiles === selection) return;

		workspaceState.selection.uncommittedFiles = selection;
	},
	selectOutline: (state: ProjectState, { selection }: { selection: Operand | null }) => {
		const workspaceState = state.workspace;
		if (
			selection &&
			workspaceState.selection.outline &&
			operandEquals(workspaceState.selection.outline, selection)
		)
			return;

		workspaceState.selection.outline = selection;
		workspaceState.selection.files = null;
		workspaceState.selection.diff = null;

		if (!selection || !isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
			workspaceState.mode = defaultOutlineMode;
	},
	selectFiles: (state: ProjectState, { selection }: { selection: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.selection.files === selection) return;

		workspaceState.selection.files = selection;
	},
	selectDiff: (state: ProjectState, { selection }: { selection: HunkOperand | null }) => {
		const workspaceState = state.workspace;
		if (
			selection &&
			workspaceState.selection.diff &&
			operandEquals(hunkOperand(workspaceState.selection.diff), hunkOperand(selection))
		)
			return;

		workspaceState.selection.diff = selection;
	},
	startRewordCommit: (state: ProjectState, { commit }: { commit: CommitOperand }) => {
		const workspaceState = state.workspace;
		const selection = commitOperand(commit);
		if (
			!workspaceState.selection.outline ||
			!operandEquals(workspaceState.selection.outline, selection)
		) {
			workspaceState.selection.outline = selection;
			workspaceState.selection.files = null;
			workspaceState.selection.diff = null;
			if (!isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
				workspaceState.mode = defaultOutlineMode;
		}

		workspaceState.mode = rewordCommitOutlineMode({ operand: commit });
	},
	startRenameBranch: (state: ProjectState, { branch }: { branch: BranchOperand }) => {
		const workspaceState = state.workspace;
		const selection = branchOperand(branch);
		if (
			!workspaceState.selection.outline ||
			!operandEquals(workspaceState.selection.outline, selection)
		) {
			workspaceState.selection.outline = selection;
			workspaceState.selection.files = null;
			workspaceState.selection.diff = null;
			if (!isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
				workspaceState.mode = defaultOutlineMode;
		}

		workspaceState.mode = renameBranchOutlineMode({ operand: branch });
	},
	updateRewrittenBranchReferences: (
		state: ProjectState,
		{ oldBranch, newBranch }: { oldBranch: BranchOperand; newBranch: BranchOperand },
	) => {
		const workspaceState = state.workspace;
		const oldBranchOperand = branchOperand(oldBranch);
		const newBranchOperand = branchOperand(newBranch);

		if (
			workspaceState.selection.outline?._tag === "Branch" &&
			operandEquals(workspaceState.selection.outline, oldBranchOperand)
		)
			workspaceState.selection.outline = newBranchOperand;

		if (
			workspaceState.commitTarget?._tag === "Branch" &&
			operandEquals(workspaceState.commitTarget, oldBranchOperand)
		) {
			workspaceState.commitTarget = branchOperand({
				branchRef: newBranch.branchRef,
			});
		}

		if (
			workspaceState.mode._tag === "RenameBranch" &&
			operandEquals(branchOperand(workspaceState.mode.operand), oldBranchOperand)
		)
			workspaceState.mode = renameBranchOutlineMode({ operand: newBranch });
	},
	enterTransferMode: (state: ProjectState, { mode }: { mode: TransferMode }) => {
		state.workspace.mode = transferOutlineMode(mode);
	},
	enterKeyboardTransferMode: (
		state: ProjectState,
		{ sources, operationType }: { sources: Array<Operand>; operationType?: OperationType },
	) => {
		const workspaceState = state.workspace;
		workspaceState.mode = transferOutlineMode(
			keyboardTransferMode({
				sources,
				operationType: operationType ?? "into",
				restoreSelection: {
					uncommittedFiles: workspaceState.selection.uncommittedFiles,
					outline: workspaceState.selection.outline,
					files: workspaceState.selection.files,
					diff: workspaceState.selection.diff,
				},
			}),
		);
	},
	enterAbsorbMode: (
		state: ProjectState,
		{ source, sourceTarget }: { source: Operand; sourceTarget: AbsorptionTarget },
	) => {
		const workspaceState = state.workspace;
		workspaceState.mode = absorbOutlineMode({
			source,
			restoreSelection: {
				uncommittedFiles: workspaceState.selection.uncommittedFiles,
				outline: workspaceState.selection.outline,
				files: workspaceState.selection.files,
				diff: workspaceState.selection.diff,
			},
			sourceTarget,
		});
	},
	updatePointerTransfer: (
		state: ProjectState,
		{ target, operationType }: { target: Operand | null; operationType: OperationType | null },
	) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.mode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) => {
				const sameTarget =
					target === null
						? mode.target === null
						: mode.target !== null && operandEquals(mode.target, target);
				if (sameTarget && mode.operationType === operationType) return;

				workspaceState.mode = transferOutlineMode(
					pointerTransferMode({
						sources: mode.sources,
						target,
						operationType,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	updateTransferOperationType: (
		state: ProjectState,
		{ operationType }: { operationType: OperationType },
	) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.mode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: mode }) => {
				workspaceState.mode = transferOutlineMode(
					keyboardTransferMode({
						sources: mode.sources,
						operationType,
						restoreSelection: mode.restoreSelection,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	exitMode: (state: ProjectState) => {
		state.workspace.mode = defaultOutlineMode;
	},
	cancelMode: (state: ProjectState) => {
		const workspaceState = state.workspace;
		const restoreSelection = Match.value(workspaceState.mode).pipe(
			Match.tags({
				Absorb: (mode) => mode.restoreSelection,
				Transfer: (mode) => (mode.value._tag === "Keyboard" ? mode.value.restoreSelection : null),
			}),
			Match.orElse(() => null),
		);
		workspaceState.mode = defaultOutlineMode;

		if (!restoreSelection) return;

		workspaceState.selection = restoreSelection;
	},
	setHighlightedChangeIds: (
		state: ProjectState,
		{ changeIds }: { changeIds: Array<string> | null },
	) => {
		state.workspace.highlightedChangeIds = changeIds ?? [];
	},
	checkCommit: (
		state: ProjectState,
		{ changeId, checked }: { changeId: string; checked: boolean },
	) => {
		const checkedChangeIds = state.workspace.checkedChangeIds;
		if (checked) checkedChangeIds[changeId] = true;
		else delete checkedChangeIds[changeId];
	},
	checkCommits: (
		state: ProjectState,
		{ changeIds, checked }: { changeIds: Array<string>; checked: boolean },
	) => {
		const checkedChangeIds = state.workspace.checkedChangeIds;
		for (const changeId of changeIds) {
			if (checked) checkedChangeIds[changeId] = true;
			else delete checkedChangeIds[changeId];
		}
	},
	setCheckedCommits: (state: ProjectState, { changeIds }: { changeIds: Array<string> }) => {
		state.workspace.checkedChangeIds = changeIds.reduce(
			(acc, changeId) => {
				acc[changeId] = true;
				return acc;
			},
			{} as Record<string, true>,
		);
	},
	clearCheckedCommits: (state: ProjectState) => {
		state.workspace.checkedChangeIds = {};
	},
	setCommitTarget: (state: ProjectState, { commitTarget }: { commitTarget: Operand | null }) => {
		state.workspace.commitTarget = commitTarget;
	},
	toggleFiles: (state: ProjectState) => {
		state.filesVisible = !state.filesVisible;
	},
	setDetailsFullWindow: (state: ProjectState, { fullWindow }: { fullWindow: boolean }) => {
		state.detailsFullWindow = fullWindow;
	},
	toggleDetailsFullWindow: (state: ProjectState) => {
		state.detailsFullWindow = !state.detailsFullWindow;
	},
	openDialog: (state: ProjectState, { dialog }: { dialog: Dialog }) => {
		state.dialog = dialog;
	},
	closeDialog: (state: ProjectState) => {
		state.dialog = { _tag: "None" };
	},
};

const selectCheckedCommits = createSelector(
	(state: ProjectState) => state.workspace.checkedChangeIds,
	(_state: ProjectState, headInfoIndex: HeadInfoIndex) => headInfoIndex,
	(checkedChangeIds, headInfoIndex) =>
		new Set(
			Object.keys(checkedChangeIds).filter(
				(changeId) => headInfoIndex.commitContextById(changeId) !== undefined,
			),
		),
);

const selectCheckedCommitOperands = createSelector(selectCheckedCommits, (checkedChangeIds) =>
	Array.from(checkedChangeIds).map((changeId) => commitOperand({ changeId })),
);

export const projectSelectors = {
	selectFilesVisible: (state: ProjectState) => state.filesVisible,
	selectCanShowFiles: (state: ProjectState) =>
		state.workspace.detailsSelectionScope !== "uncommitted-files",
	selectDetailsFullWindow: (state: ProjectState) => state.detailsFullWindow,
	selectDetailsSelectionScope: (state: ProjectState) => state.workspace.detailsSelectionScope,
	selectDialogState: (state: ProjectState) => state.dialog,
	selectSelectionUncommittedFiles: (
		state: ProjectState,
		navigationIndex: NavigationIndex<string>,
	) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.uncommittedFiles,
			(path) => path,
		),
	selectIsSelectedOutline: (
		state: ProjectState,
		navigationIndex: NavigationIndex<Operand>,
		operand: Operand,
	) => {
		const selection = resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.outline,
			operandIdentityKey,
		);
		return selection !== null && operandEquals(selection, operand);
	},
	selectSelectionOutline: (state: ProjectState, navigationIndex: NavigationIndex<Operand>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.outline,
			operandIdentityKey,
		),
	selectSelectionFiles: (state: ProjectState, navigationIndex: NavigationIndex<string>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.files,
			(item) => item,
		),
	selectSelectionDiff: (state: ProjectState, navigationIndex: NavigationIndex<HunkOperand>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.diff,
			hunkOperandIdentityKey,
		),
	selectOutlineModeState: (state: ProjectState) => state.workspace.mode,
	selectHighlightedChangeIds: (state: ProjectState) => state.workspace.highlightedChangeIds,
	selectCommitChecked: (state: ProjectState, changeId: string) =>
		state.workspace.checkedChangeIds[changeId] === true,
	selectCheckedCommits,
	selectCheckedCommitOperands,
	selectCheckedCommitCount: (state: ProjectState, headInfoIndex: HeadInfoIndex) =>
		selectCheckedCommits(state, headInfoIndex).size,
	selectHasCheckedCommits: (state: ProjectState, headInfoIndex: HeadInfoIndex) =>
		selectCheckedCommits(state, headInfoIndex).size > 0,
	selectCommitTarget: (state: ProjectState) => state.workspace.commitTarget,
};
