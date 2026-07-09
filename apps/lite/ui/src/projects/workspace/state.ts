import { type OperationType } from "#ui/operations/operation.ts";
import { AbsorptionTarget, type RefInfo, type RelativeTo } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	branchOperand,
	commitOperand,
	hunkOperand,
	operandEquals,
	type BranchOperand,
	type CommitOperand,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
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
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";

export type SelectionState = {
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

const createInitialSelectionState = (): SelectionState => ({
	outline: null,
	files: null,
	diff: null,
});

export type WorkspaceState = {
	checkedCommitIds: Record<string, true>;
	commitTarget: RelativeTo | null;
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	selection: SelectionState;
};

export const createInitialState = (): WorkspaceState => ({
	checkedCommitIds: {},
	commitTarget: null,
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selection: createInitialSelectionState(),
});

export const enterTransferMode = (state: WorkspaceState, mode: TransferMode) => {
	state.mode = transferOutlineMode(mode);
};

export const enterKeyboardTransferMode = (
	state: WorkspaceState,
	source: Operand,
	operationType?: OperationType,
) => {
	state.mode = transferOutlineMode(
		keyboardTransferMode({
			source,
			operationType: operationType ?? "into",
			restoreSelection: {
				outline: state.selection.outline,
				files: state.selection.files,
				diff: state.selection.diff,
			},
		}),
	);
};

export const enterAbsorbMode = (
	state: WorkspaceState,
	source: Operand,
	sourceTarget: AbsorptionTarget,
) => {
	state.mode = absorbOutlineMode({
		source,
		restoreSelection: {
			outline: state.selection.outline,
			files: state.selection.files,
			diff: state.selection.diff,
		},
		sourceTarget,
	});
};

export const updatePointerTransfer = (
	state: WorkspaceState,
	target: Operand | null,
	operationType: OperationType | null,
) => {
	Match.value(state.mode).pipe(
		Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) => {
			const sameTarget =
				target === null
					? mode.target === null
					: mode.target !== null && operandEquals(mode.target, target);
			if (sameTarget && mode.operationType === operationType) return;

			state.mode = transferOutlineMode(
				pointerTransferMode({
					source: mode.source,
					target,
					operationType,
				}),
			);
		}),
		Match.orElse(() => {}),
	);
};

export const updateTransferOperationType = (
	state: WorkspaceState,
	operationType: OperationType,
) => {
	Match.value(state.mode).pipe(
		Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: mode }) => {
			state.mode = transferOutlineMode(
				keyboardTransferMode({
					source: mode.source,
					operationType,
					restoreSelection: mode.restoreSelection,
				}),
			);
		}),
		Match.orElse(() => {}),
	);
};

export const exitMode = (state: WorkspaceState) => {
	state.mode = defaultOutlineMode;
};

export const cancelMode = (state: WorkspaceState) => {
	const restoreSelection = Match.value(state.mode).pipe(
		Match.tags({
			Absorb: (mode) => mode.restoreSelection,
			Transfer: (mode) => (mode.value._tag === "Keyboard" ? mode.value.restoreSelection : null),
		}),
		Match.orElse(() => null),
	);
	exitMode(state);

	if (!restoreSelection) return;

	state.selection = restoreSelection;
};

export const selectOutline = (state: WorkspaceState, selection: Operand | null) => {
	if (selection && state.selection.outline && operandEquals(state.selection.outline, selection))
		return;

	state.selection.outline = selection;
	state.selection.files = null;
	state.selection.diff = null;

	if (!selection || !isValidOutlineModeForSelection({ mode: state.mode, selection }))
		exitMode(state);
};

export const selectFiles = (state: WorkspaceState, selection: string | null) => {
	if (state.selection.files === selection) return;

	state.selection.files = selection;
};

export const selectDiff = (state: WorkspaceState, selection: HunkOperand | null) => {
	if (
		selection &&
		state.selection.diff &&
		operandEquals(hunkOperand(state.selection.diff), hunkOperand(selection))
	)
		return;

	state.selection.diff = selection;
};

export const setHighlightedCommitIds = (state: WorkspaceState, commitIds: Array<string> | null) => {
	state.highlightedCommitIds = commitIds ?? [];
};

export const setCommitChecked = (state: WorkspaceState, commitId: string, checked: boolean) => {
	if (checked) state.checkedCommitIds[commitId] = true;
	else delete state.checkedCommitIds[commitId];
};

export const setCommitsChecked = (
	state: WorkspaceState,
	commitIds: Array<string>,
	checked: boolean,
) => {
	for (const commitId of commitIds) {
		if (checked) state.checkedCommitIds[commitId] = true;
		else delete state.checkedCommitIds[commitId];
	}
};

export const clearCheckedCommits = (state: WorkspaceState) => {
	state.checkedCommitIds = {};
};

export const setCommitTarget = (state: WorkspaceState, commitTarget: RelativeTo | null) => {
	state.commitTarget = commitTarget;
};

const rewrittenCommitOperand = ({
	commit,
	headInfo,
	replacedCommits,
}: {
	commit: CommitOperand;
	headInfo: RefInfo;
	replacedCommits: Record<string, string>;
}): CommitOperand | null => {
	const commitId = replacedCommits[commit.commitId];
	if (commitId === undefined) return null;

	const commitCtx = getHeadInfoIndex(headInfo).commitContextById(commitId);
	if (commitCtx?.stack.id == null) return null;

	return {
		stackId: commitCtx.stack.id,
		commitId: commitCtx.commit.id,
		changeId: commitCtx.commit.changeId,
	};
};

export const rewrittenCommitSelection = ({
	selection,
	replacedCommits,
	headInfo,
}: {
	selection: Operand | null;
	replacedCommits: Record<string, string>;
	headInfo: RefInfo;
}): Operand | null => {
	if (selection?._tag !== "Commit") return selection;

	const commit = rewrittenCommitOperand({
		commit: selection,
		replacedCommits,
		headInfo,
	});
	if (!commit) return selection;

	return commitOperand(commit);
};

export const startRenameBranch = (state: WorkspaceState, branch: BranchOperand) => {
	selectOutline(state, branchOperand(branch));
	state.mode = renameBranchOutlineMode({ operand: branch });
};

export const startRewordCommit = (state: WorkspaceState, commit: CommitOperand) => {
	selectOutline(state, commitOperand(commit));
	state.mode = rewordCommitOutlineMode({ operand: commit });
};

export const selectSelectionOutlineState = (state: WorkspaceState): Operand | null =>
	state.selection.outline;

export const selectSelectionFilesState = (state: WorkspaceState): string | null =>
	state.selection.files;

export const selectSelectionDiffState = (state: WorkspaceState): HunkOperand | null =>
	state.selection.diff;

export const selectMode = (state: WorkspaceState): OutlineMode => state.mode;

export const selectHighlightedCommitIds = (state: WorkspaceState): Array<string> =>
	state.highlightedCommitIds;

export const selectCommitChecked = (state: WorkspaceState, commitId: string): boolean =>
	state.checkedCommitIds[commitId] === true;

export const selectCheckedCommitCount = (state: WorkspaceState): number =>
	Object.keys(state.checkedCommitIds).length;

export const selectHasCheckedCommits = (state: WorkspaceState): boolean =>
	selectCheckedCommitCount(state) > 0;

export const selectCommitTarget = (state: WorkspaceState): RelativeTo | null => state.commitTarget;
