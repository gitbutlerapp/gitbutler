import { Match } from "effect";
import {
	BranchOperand,
	branchOperand,
	CommitOperand,
	commitOperand,
	operandContains,
	operandEquals,
	type Operand,
} from "#ui/operands.ts";
import { getOperation, getOperations, OperationType } from "#ui/operations/operation.ts";
import { filterNavigationIndex, NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { CommitAbsorption } from "@gitbutler/but-sdk";

/** @public */
export type AbsorbOperationMode = {
	source: Operand;
	absorptionPlan: Array<CommitAbsorption>;
};
/** @public */
export type CutOperationMode = { source: Operand; operationType: OperationType };
/** @public */
export type DragAndDropOperationMode = { source: Operand; operationType?: OperationType };
export type OperationMode =
	| ({ _tag: "Absorb" } & AbsorbOperationMode)
	| ({ _tag: "Cut" } & CutOperationMode)
	| ({ _tag: "DragAndDrop" } & DragAndDropOperationMode);

/** @public */
export const absorbOperationMode = ({
	source,
	absorptionPlan,
}: AbsorbOperationMode): OperationMode => ({
	_tag: "Absorb",
	source,
	absorptionPlan,
});

/** @public */
export const cutOperationMode = ({ source, operationType }: CutOperationMode): OperationMode => ({
	_tag: "Cut",
	source,
	operationType,
});

/** @public */
export const dragAndDropOperationMode = ({
	source,
	operationType,
}: DragAndDropOperationMode): OperationMode => ({
	_tag: "DragAndDrop",
	source,
	operationType,
});

/** @public */
export type RewordCommitOutlineMode = { operand: CommitOperand };
/** @public */
export type RenameBranchOutlineMode = { operand: BranchOperand };
export type OutlineMode =
	| { _tag: "Default" }
	| ({ _tag: "RewordCommit" } & RewordCommitOutlineMode)
	| ({ _tag: "RenameBranch" } & RenameBranchOutlineMode)
	| { _tag: "Operation"; value: OperationMode };

/** @public */
export const defaultOutlineMode: OutlineMode = {
	_tag: "Default",
};

/** @public */
export const operationOutlineMode = (value: OperationMode): OutlineMode => ({
	_tag: "Operation",
	value,
});

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

export const getOperationMode = (mode: OutlineMode): OperationMode | undefined =>
	Match.value(mode).pipe(
		Match.withReturnType<OperationMode | undefined>(),
		Match.tags({ Operation: ({ value }) => value }),
		Match.orElse(() => undefined),
	);

const operationModeToOperationType = (operationMode: OperationMode): OperationType | undefined =>
	Match.value(operationMode).pipe(
		Match.withReturnType<OperationType | undefined>(),
		Match.tags({
			Absorb: () => undefined,
			Cut: ({ operationType }) => operationType,
			DragAndDrop: ({ operationType }) => operationType,
		}),
		Match.exhaustive,
	);

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
			Operation: () => true,
			RewordCommit: (mode) => operandEquals(selection, commitOperand(mode.operand)),
			RenameBranch: (mode) => operandEquals(selection, branchOperand(mode.operand)),
		}),
	);

export const getBinaryOperation = ({ mode, target }: { mode: OperationMode; target: Operand }) =>
	getOperation({
		source: mode.source,
		target,
		operationType: operationModeToOperationType(mode),
	});

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.rub || !!operations.moveAbove || !!operations.moveBelow;
};

export const isOperationModeCandidateTarget = ({
	mode,
	target,
}: {
	mode: OperationMode;
	target: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Absorb: ({ absorptionPlan }) =>
				absorptionPlan.some(({ stackId, commitId }) =>
					operandEquals(commitOperand({ stackId, commitId }), target),
				),
			DragAndDrop: ({ source }) => hasAnyOperation(source, target),
			Cut: ({ source }) => hasAnyOperation(source, target),
		}),
	);

export const filterNavigationIndexForOutlineMode = ({
	navigationIndex: navigationIndexUnfiltered,
	outlineMode,
}: {
	navigationIndex: NavigationIndex;
	outlineMode: OutlineMode;
}) =>
	Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => navigationIndexUnfiltered,
			Operation: ({ value: operationMode }) =>
				filterNavigationIndex(
					navigationIndexUnfiltered,
					(operand) =>
						operandContains(operand, operationMode.source) ||
						isOperationModeCandidateTarget({ mode: operationMode, target: operand }),
				),
			RenameBranch: (x) =>
				filterNavigationIndex(navigationIndexUnfiltered, (operand) =>
					operandEquals(operand, branchOperand(x.operand)),
				),
			RewordCommit: (x) =>
				filterNavigationIndex(navigationIndexUnfiltered, (operand) =>
					operandEquals(operand, commitOperand(x.operand)),
				),
		}),
	);
