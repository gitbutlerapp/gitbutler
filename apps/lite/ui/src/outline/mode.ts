import { Match } from "effect";
import {
	BranchOperand,
	branchOperand,
	CommitOperand,
	commitOperand,
	operandEquals,
	type Operand,
} from "#ui/operands.ts";
import { TransferPosition } from "#ui/operations/transfer-operation.ts";
import type { SelectionState } from "#ui/projects/project.ts";
import { AbsorptionTarget } from "@gitbutler/but-sdk";

/** @public */
export type AbsorbMode = {
	source: Operand;
	sourceTarget: AbsorptionTarget;
	restoreSelection: SelectionState;
};

/** @public */
export type KeyboardTransferMode = {
	source: Operand;
	position: TransferPosition;
	restoreSelection: SelectionState;
};

/** @public */
export type PointerTransferMode = {
	source: Operand;
	target: Operand | null;
	position: TransferPosition | null;
};

/** @public */
export type TransferMode =
	| ({ _tag: "Keyboard" } & KeyboardTransferMode)
	| ({ _tag: "Pointer" } & PointerTransferMode);

/** @public */
export const keyboardTransferMode = ({
	source,
	position,
	restoreSelection,
}: KeyboardTransferMode): TransferMode => ({
	_tag: "Keyboard",
	source,
	position,
	restoreSelection,
});

/** @public */
export const pointerTransferMode = ({
	source,
	target,
	position,
}: PointerTransferMode): TransferMode => ({
	_tag: "Pointer",
	source,
	target,
	position,
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
export const transferOutlineMode = (mode: TransferMode): OutlineMode => ({
	_tag: "Transfer",
	value: mode,
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
	| { _tag: "Transfer"; value: TransferMode };

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
