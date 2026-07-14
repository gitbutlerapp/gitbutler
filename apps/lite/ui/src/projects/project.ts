import { bytesEqual } from "#ui/api/bytes.ts";
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
import type { AbsorptionTarget, RefInfo, RelativeTo } from "@gitbutler/but-sdk";
import { Match } from "effect";

export type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings" };

export type SelectionState = {
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

type WorkspaceState = {
	checkedCommitIds: Record<string, true>;
	commitTarget: RelativeTo | null;
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	selection: SelectionState;
};

const createInitialSelectionState = (): SelectionState => ({
	outline: null,
	files: null,
	diff: null,
});

const createInitialWorkspaceState = (): WorkspaceState => ({
	checkedCommitIds: {},
	commitTarget: null,
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selection: createInitialSelectionState(),
});

export type ProjectState = {
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
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
			workspaceState.commitTarget?.type === "referenceBytes" &&
			bytesEqual(workspaceState.commitTarget.subject, oldBranch.branchRef)
		) {
			workspaceState.commitTarget = {
				type: "referenceBytes",
				subject: newBranch.branchRef,
			};
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
		{ source, operationType }: { source: Operand; operationType?: OperationType },
	) => {
		const workspaceState = state.workspace;
		workspaceState.mode = transferOutlineMode(
			keyboardTransferMode({
				source,
				operationType: operationType ?? "into",
				restoreSelection: {
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
						source: mode.source,
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
						source: mode.source,
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
	setHighlightedCommitIds: (
		state: ProjectState,
		{ commitIds }: { commitIds: Array<string> | null },
	) => {
		state.workspace.highlightedCommitIds = commitIds ?? [];
	},
	setCommitChecked: (
		state: ProjectState,
		{ commitId, checked }: { commitId: string; checked: boolean },
	) => {
		const checkedCommitIds = state.workspace.checkedCommitIds;
		if (checked) checkedCommitIds[commitId] = true;
		else delete checkedCommitIds[commitId];
	},
	setCommitsChecked: (
		state: ProjectState,
		{ commitIds, checked }: { commitIds: Array<string>; checked: boolean },
	) => {
		const checkedCommitIds = state.workspace.checkedCommitIds;
		for (const commitId of commitIds) {
			if (checked) checkedCommitIds[commitId] = true;
			else delete checkedCommitIds[commitId];
		}
	},
	clearCheckedCommits: (state: ProjectState) => {
		state.workspace.checkedCommitIds = {};
	},
	setCommitTarget: (state: ProjectState, { commitTarget }: { commitTarget: RelativeTo | null }) => {
		state.workspace.commitTarget = commitTarget;
	},
	updateRewrittenCommitReferences: (
		state: ProjectState,
		{ replacedCommits, headInfo }: { replacedCommits: Record<string, string>; headInfo: RefInfo },
	) => {
		const workspaceState = state.workspace;
		const commit = rewrittenCommitSelection({
			selection: workspaceState.selection.outline,
			replacedCommits,
			headInfo,
		});
		if (commit) workspaceState.selection.outline = commit;

		if (workspaceState.commitTarget?.type === "commit") {
			const commitId = replacedCommits[workspaceState.commitTarget.subject];
			if (commitId !== undefined)
				workspaceState.commitTarget = { type: "commit", subject: commitId };
		}

		for (const oldId of Object.keys(workspaceState.checkedCommitIds)) {
			const newId = replacedCommits[oldId];
			if (newId !== undefined) {
				delete workspaceState.checkedCommitIds[oldId];
				workspaceState.checkedCommitIds[newId] = true;
			}
		}

		if (workspaceState.mode._tag === "RewordCommit") {
			const commit = rewrittenCommitOperand({
				commit: workspaceState.mode.operand,
				replacedCommits,
				headInfo,
			});
			if (commit) workspaceState.mode = rewordCommitOutlineMode({ operand: commit });
		}
	},
};

export const projectSelectors = {
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
	selectHighlightedCommitIds: (state: ProjectState) => state.workspace.highlightedCommitIds,
	selectCommitChecked: (state: ProjectState, commitId: string) =>
		state.workspace.checkedCommitIds[commitId] === true,
	selectCheckedCommitCount: (state: ProjectState) =>
		Object.keys(state.workspace.checkedCommitIds).length,
	selectHasCheckedCommits: (state: ProjectState) =>
		Object.keys(state.workspace.checkedCommitIds).length > 0,
	selectCommitTarget: (state: ProjectState) => state.workspace.commitTarget,
};
