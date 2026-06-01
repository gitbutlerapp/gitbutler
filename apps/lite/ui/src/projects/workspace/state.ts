import { type OperationType } from "#ui/operations/operation.ts";
import { refNamesEqual } from "#ui/api/ref-name.ts";
import { AbsorptionTarget, type RefInfo, type RelativeTo } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	operandEquals,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import {
	absorbOutlineMode,
	defaultOutlineMode,
	isValidOutlineModeForSelection,
	keyboardTransferOperationMode,
	pointerTransferOperationMode,
	renameBranchOutlineMode,
	rewordCommitOutlineMode,
	transferOutlineMode,
	type OutlineMode,
	type TransferOperationMode,
} from "#ui/outline/mode.ts";
import { findCommitStackId } from "#ui/api/ref-info.ts";

export type SelectionState = {
	outline: Operand;
	files: Operand;
};

export const defaultOutlineSelection = changesSectionOperand;

const createInitialSelectionState = (): SelectionState => ({
	outline: defaultOutlineSelection,
	files: defaultOutlineSelection,
});

export type WorkspaceState = {
	commitTarget: RelativeTo | null;
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	selection: SelectionState;
};

export const createInitialState = (): WorkspaceState => ({
	commitTarget: null,
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selection: createInitialSelectionState(),
});

export const initialState: WorkspaceState = createInitialState();

export const enterTransferMode = (state: WorkspaceState, mode: TransferOperationMode) => {
	state.mode = transferOutlineMode({
		value: mode,
		restoreSelection: {
			outline: state.selection.outline,
			files: state.selection.files,
		},
	});
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
		Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, (mode) => {
			if (target !== null && !operandEquals(state.selection.outline, target))
				selectOutline(state, target);

			if (mode.value.operationType === operationType) return;

			state.mode = transferOutlineMode({
				value: pointerTransferOperationMode({
					source: mode.value.source,
					operationType,
				}),
				restoreSelection: mode.restoreSelection,
			});
		}),
		Match.orElse(() => {}),
	);
};

export const updateTransferOperationType = (
	state: WorkspaceState,
	operationType: OperationType,
) => {
	Match.value(state.mode).pipe(
		Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, (mode) => {
			state.mode = transferOutlineMode({
				value: keyboardTransferOperationMode({
					source: mode.value.source,
					operationType,
				}),
				restoreSelection: mode.restoreSelection,
			});
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
			Transfer: (mode) => mode.restoreSelection,
		}),
		Match.orElse(() => null),
	);
	exitMode(state);

	if (!restoreSelection) return;

	state.selection = restoreSelection;
};

export const selectOutline = (state: WorkspaceState, selection: Operand) => {
	state.selection.outline = selection;
	state.selection.files = selection;

	if (!isValidOutlineModeForSelection({ mode: state.mode, selection })) exitMode(state);
};

export const selectFiles = (state: WorkspaceState, selection: Operand) => {
	state.selection.files = selection;
};

export const setHighlightedCommitIds = (state: WorkspaceState, commitIds: Array<string> | null) => {
	state.highlightedCommitIds = commitIds ?? [];
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

	const stackId = findCommitStackId(headInfo, commitId);
	if (stackId === null) return null;

	return { stackId, commitId };
};

export const updateRewrittenCommitReferences = (
	state: WorkspaceState,
	replacedCommits: Record<string, string>,
	headInfo: RefInfo,
) => {
	if (state.selection.outline._tag === "Commit") {
		const commit = rewrittenCommitOperand({
			commit: state.selection.outline,
			replacedCommits,
			headInfo,
		});
		if (commit) state.selection.outline = commitOperand(commit);
	}

	if (state.selection.files._tag === "Commit") {
		const commit = rewrittenCommitOperand({
			commit: state.selection.files,
			replacedCommits,
			headInfo,
		});
		if (commit) state.selection.files = commitOperand(commit);
	}

	if (state.commitTarget?.type === "commit") {
		const commitId = replacedCommits[state.commitTarget.subject];
		if (commitId !== undefined) state.commitTarget = { type: "commit", subject: commitId };
	}
};

export const startRenameBranch = (state: WorkspaceState, branch: BranchOperand) => {
	selectOutline(state, branchOperand(branch));
	state.mode = renameBranchOutlineMode({ operand: branch });
};

export const updateRewrittenBranchReferences = (
	state: WorkspaceState,
	oldBranch: BranchOperand,
	newBranch: BranchOperand,
) => {
	const oldBranchOperand = branchOperand(oldBranch);
	const newBranchOperand = branchOperand(newBranch);

	if (
		state.selection.outline._tag === "Branch" &&
		operandEquals(state.selection.outline, oldBranchOperand)
	)
		state.selection.outline = newBranchOperand;

	if (
		state.selection.files._tag === "Branch" &&
		operandEquals(state.selection.files, oldBranchOperand)
	)
		state.selection.files = newBranchOperand;

	if (
		state.commitTarget?.type === "referenceBytes" &&
		refNamesEqual(state.commitTarget.subject, oldBranch.branchRef)
	)
		state.commitTarget = { type: "referenceBytes", subject: newBranch.branchRef };
};

export const startRewordCommit = (state: WorkspaceState, commit: CommitOperand) => {
	selectOutline(state, commitOperand(commit));
	state.mode = rewordCommitOutlineMode({ operand: commit });
};

export const selectSelectionOutlineState = (state: WorkspaceState): Operand =>
	state.selection.outline;

export const selectSelectionFilesState = (state: WorkspaceState): Operand => state.selection.files;

export const selectMode = (state: WorkspaceState): OutlineMode => state.mode;

export const selectHighlightedCommitIds = (state: WorkspaceState): Array<string> =>
	state.highlightedCommitIds;

export const selectCommitTarget = (state: WorkspaceState): RelativeTo | null => state.commitTarget;
