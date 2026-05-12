import { type OperationType } from "#ui/operations/operation.ts";
import { CommitAbsorption } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import {
	absorbOperationMode,
	cutOperationMode,
	defaultOutlineMode,
	dragAndDropOperationMode,
	getOperationMode,
	isValidOutlineModeForSelection,
	operationOutlineMode,
	renameBranchOutlineMode,
	rewordCommitOutlineMode,
	type OutlineMode,
} from "#ui/outline/mode.ts";

type SelectionState = {
	outline: Operand;
	files: Operand;
};

export const defaultOutlineSelection = changesSectionOperand;

const createInitialSelectionState = (): SelectionState => ({
	outline: defaultOutlineSelection,
	files: defaultOutlineSelection,
});

export type WorkspaceState = {
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	replacedCommits: Record<string, string>;
	selection: SelectionState;
};

export const createInitialState = (): WorkspaceState => ({
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	replacedCommits: {},
	selection: createInitialSelectionState(),
});

export const initialState: WorkspaceState = createInitialState();

export const enterCutMode = (
	state: WorkspaceState,
	source: Operand,
	operationType: OperationType,
) => {
	state.mode = operationOutlineMode(cutOperationMode({ source, operationType }));
};

export const enterAbsorbMode = (
	state: WorkspaceState,
	source: Operand,
	absorptionPlan: Array<CommitAbsorption>,
) => {
	state.mode = operationOutlineMode(absorbOperationMode({ source, absorptionPlan }));
};

export const enterDragAndDropMode = (state: WorkspaceState, source: Operand) => {
	state.mode = operationOutlineMode(dragAndDropOperationMode({ source }));
};

export const updateDragAndDropMode = (
	state: WorkspaceState,
	operationType: OperationType | undefined,
) => {
	Match.value(state.mode).pipe(
		Match.when({ _tag: "Operation", value: { _tag: "DragAndDrop" } }, (mode) => {
			if (mode.value.operationType === operationType) return;

			state.mode = operationOutlineMode(
				dragAndDropOperationMode({ source: mode.value.source, operationType }),
			);
		}),
		Match.orElse(() => {}),
	);
};

export const updateCutMode = (state: WorkspaceState, operationType: OperationType) => {
	Match.value(state.mode).pipe(
		Match.when({ _tag: "Operation", value: { _tag: "Cut" } }, (mode) => {
			state.mode = operationOutlineMode(
				cutOperationMode({ source: mode.value.source, operationType }),
			);
		}),
		Match.orElse(() => {}),
	);
};

export const exitMode = (state: WorkspaceState) => {
	state.mode = defaultOutlineMode;
};

export const selectOutline = (state: WorkspaceState, selection: Operand) => {
	state.selection.outline = selection;
	state.selection.files = selection;

	if (!isValidOutlineModeForSelection({ mode: state.mode, selection }))
		state.mode = defaultOutlineMode;
};

export const selectFiles = (state: WorkspaceState, selection: Operand) => {
	state.selection.files = selection;
};

export const setHighlightedCommitIds = (
	state: WorkspaceState,
	commitIds: Array<string> | undefined,
) => {
	state.highlightedCommitIds = commitIds ?? [];
};

export const addReplacedCommits = (
	state: WorkspaceState,
	replacedCommits: Record<string, string>,
) => {
	state.replacedCommits = { ...state.replacedCommits, ...replacedCommits };
};

export const startRenameBranch = (state: WorkspaceState, branch: BranchOperand) => {
	selectOutline(state, branchOperand(branch));
	state.mode = renameBranchOutlineMode({ operand: branch });
};

export const startRewordCommit = (state: WorkspaceState, commit: CommitOperand) => {
	selectOutline(state, commitOperand(commit));
	state.mode = rewordCommitOutlineMode({ operand: commit });
};

export const selectSelectionOutlineState = (state: WorkspaceState): Operand =>
	state.selection.outline;

export const selectSelectionFilesState = (state: WorkspaceState): Operand => state.selection.files;

export const selectMode = (state: WorkspaceState): OutlineMode => state.mode;

export const selectOperationMode = (state: WorkspaceState) => getOperationMode(state.mode);

export const selectHighlightedCommitIds = (state: WorkspaceState): Array<string> =>
	state.highlightedCommitIds;

export const selectReplacedCommits = (state: WorkspaceState): Record<string, string> =>
	state.replacedCommits;
