import { Match } from "effect";
import {
	type BranchOperand,
	branchOperand,
	type CommitOperand,
	commitOperand,
	operandEquals,
	type Operand,
	uncommittedChangesOperand,
} from "#ui/operands.ts";
import type { Placement } from "#ui/operations/operation.ts";
import type { WorkspaceCursorSnapshot } from "#ui/cursors.ts";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";

/** @public */
export type AbsorbMode = {
	source: Operand;
	sourceTarget: AbsorptionTarget;
	restoreSelection: WorkspaceCursorSnapshot;
};

/** @public */
export type KeyboardTransferMode = {
	sources: Array<Operand>;
	placement: Placement;
	restoreSelection: WorkspaceCursorSnapshot;
};

/** @public */
export type PointerTransferMode = {
	sources: Array<Operand>;
	target: Operand | null;
	placement: Placement | null;
};

/** @public */
export type TransferMode =
	| ({ _tag: "Keyboard" } & KeyboardTransferMode)
	| ({ _tag: "Pointer" } & PointerTransferMode);

/** @public */
export const keyboardTransferMode = ({
	sources,
	placement,
	restoreSelection,
}: KeyboardTransferMode): TransferMode => ({
	_tag: "Keyboard",
	sources,
	placement,
	restoreSelection,
});

/** @public */
export const pointerTransferMode = ({
	sources,
	target,
	placement,
}: PointerTransferMode): TransferMode => ({
	_tag: "Pointer",
	sources,
	target,
	placement,
});

export const getTransferTarget = (
	mode: TransferMode,
	outlineSelection: Operand | null,
	workspaceList: "stacks" | "uncommitted",
): Operand | null =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Pointer: (mode) => mode.target,
			Keyboard: () =>
				Match.value(workspaceList).pipe(
					Match.when("uncommitted", () => uncommittedChangesOperand),
					Match.when("stacks", () => outlineSelection),
					Match.exhaustive,
				),
		}),
	);

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

export const getOperationSources = (mode: OutlineMode): Array<Operand> | null =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => null,
			Absorb: (x) => [x.source],
			Transfer: (x) => x.value.sources,
			RenameBranch: () => null,
			RewordCommit: () => null,
		}),
	);
