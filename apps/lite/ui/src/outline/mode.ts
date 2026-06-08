import { Match } from "effect";
import {
	BranchOperand,
	branchOperand,
	CommitOperand,
	commitOperand,
	operandContains,
	operandEquals,
	operandIdentityKey,
	type Operand,
} from "#ui/operands.ts";
import { getOperation, getOperations, OperationType } from "#ui/operations/operation.ts";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { SelectionState } from "#ui/projects/workspace/state.ts";

/** @public */
export type AbsorbMode = {
	source: Operand;
	sourceTarget: AbsorptionTarget;
	restoreSelection: SelectionState;
};

/** @public */
export type TransferMode = {
	value: TransferOperationMode;
	restoreSelection: SelectionState;
};

/** @public */
export type KeyboardTransferOperationMode = {
	source: Operand;
	operationType: OperationType;
};

/** @public */
export type PointerTransferOperationMode = {
	source: Operand;
	operationType: OperationType | null;
};

/** @public */
export type TransferOperationMode =
	| ({ _tag: "Keyboard" } & KeyboardTransferOperationMode)
	| ({ _tag: "Pointer" } & PointerTransferOperationMode);

/** @public */
export const keyboardTransferOperationMode = ({
	source,
	operationType,
}: KeyboardTransferOperationMode): TransferOperationMode => ({
	_tag: "Keyboard",
	source,
	operationType,
});

/** @public */
export const pointerTransferOperationMode = ({
	source,
	operationType,
}: PointerTransferOperationMode): TransferOperationMode => ({
	_tag: "Pointer",
	source,
	operationType,
});

/** @public */
export const absorbOutlineMode = ({
	source,
	restoreSelection,
	sourceTarget,
}: AbsorbMode): OutlineMode => ({
	_tag: "Absorb",
	source,
	restoreSelection,
	sourceTarget,
});

/** @public */
export const transferOutlineMode = ({ value, restoreSelection }: TransferMode): OutlineMode => ({
	_tag: "Transfer",
	restoreSelection,
	value,
});

/** @public */
export type RewordCommitOutlineMode = { operand: CommitOperand };
/** @public */
export type RenameBranchOutlineMode = { operand: BranchOperand };
export type OutlineMode =
	| { _tag: "Default" }
	| ({ _tag: "RewordCommit" } & RewordCommitOutlineMode)
	| ({ _tag: "RenameBranch" } & RenameBranchOutlineMode)
	| ({ _tag: "Absorb" } & AbsorbMode)
	| ({ _tag: "Transfer" } & TransferMode);

/** @public */
export const defaultOutlineMode: OutlineMode = {
	_tag: "Default",
};

/** @public */
export const rewordCommitOutlineMode = ({ operand }: RewordCommitOutlineMode): OutlineMode => ({
	_tag: "RewordCommit",
	operand,
});

/** @public */
export const renameBranchOutlineMode = ({ operand }: RenameBranchOutlineMode): OutlineMode => ({
	_tag: "RenameBranch",
	operand,
});

export const isValidOutlineModeForSelection = ({
	mode,
	selection,
}: {
	mode: OutlineMode;
	selection: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Absorb: () => true,
			Transfer: () => true,
			RewordCommit: (mode) => operandEquals(selection, commitOperand(mode.operand)),
			RenameBranch: (mode) => operandEquals(selection, branchOperand(mode.operand)),
		}),
	);

export const getTransferOperation = ({
	mode,
	target,
}: {
	mode: TransferOperationMode;
	target: Operand;
}) => {
	const { operationType } = mode;
	if (operationType === null) return null;
	return getOperation({
		source: mode.source,
		target,
		operationType,
	});
};

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.squash || !!operations.moveAbove || !!operations.moveBelow;
};

export const filterNavigationItemsForOutlineMode = ({
	items,
	outlineMode,
	absorptionTargetKeys,
}: {
	items: Array<Operand>;
	outlineMode: OutlineMode;
	absorptionTargetKeys: ReadonlySet<string>;
}) =>
	Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => items,
			Absorb: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.source) ||
						absorptionTargetKeys.has(operandIdentityKey(operand)),
				),
			Transfer: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.value.source) ||
						hasAnyOperation(activeMode.value.source, operand),
				),
			RenameBranch: (x) =>
				items.filter((operand) => operandEquals(operand, branchOperand(x.operand))),
			RewordCommit: (x) =>
				items.filter((operand) => operandEquals(operand, commitOperand(x.operand))),
		}),
	);

export const getOperationSource = (mode: OutlineMode): Operand | null =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => null,
			Absorb: (x) => x.source,
			Transfer: (x) => x.value.source,
			RenameBranch: () => null,
			RewordCommit: () => null,
		}),
	);
