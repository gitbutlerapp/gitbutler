import { Match } from "effect";
import { branchItem, commitItem, itemEquals, type Item } from "./Item.ts";
import { navigationIndexIncludes, type NavigationIndex } from "./WorkspaceModel.ts";
import {
	absorbOperation,
	getOperation,
	getOperations,
	OperationType,
	type AbsorbOperation,
	type Operation,
} from "#ui/Operation.ts";
import { CommitAbsorption } from "@gitbutler/but-sdk";

/** @public */
export type RubOperationMode = { source: Item };
/** @public */
export type MoveOperationMode = { source: Item };
/** @public */
export type AbsorbOperationMode = Omit<AbsorbOperation, "_tag">;
/** @public */
export type DragAndDropOperationMode = { source: Item; operationType: OperationType | null };
export type OperationMode =
	| ({ _tag: "Absorb" } & AbsorbOperationMode)
	| ({ _tag: "Rub" } & RubOperationMode)
	| ({ _tag: "Move" } & MoveOperationMode)
	| ({ _tag: "DragAndDrop" } & DragAndDropOperationMode);

/** @public */
export const rubOperationMode = ({ source }: RubOperationMode): OperationMode => ({
	_tag: "Rub",
	source,
});

/** @public */
export const moveOperationMode = ({ source }: MoveOperationMode): OperationMode => ({
	_tag: "Move",
	source,
});

/** @public */
export const absorbOperationMode = ({
	source,
	target,
	absorptionPlan,
}: AbsorbOperationMode): OperationMode => ({
	_tag: "Absorb",
	source,
	target,
	absorptionPlan,
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
export type RewordCommitWorkspaceMode = { stackId: string; commitId: string };
/** @public */
export type RenameBranchWorkspaceMode = { stackId: string; branchRef: Array<number> };
export type WorkspaceMode =
	| { _tag: "Default" }
	| ({ _tag: "RewordCommit" } & RewordCommitWorkspaceMode)
	| ({ _tag: "RenameBranch" } & RenameBranchWorkspaceMode)
	| { _tag: "Operation"; value: OperationMode };

/** @public */
export const defaultWorkspaceMode: WorkspaceMode = {
	_tag: "Default",
};

/** @public */
export const operationWorkspaceMode = (value: OperationMode): WorkspaceMode => ({
	_tag: "Operation",
	value,
});

/** @public */
export const rewordCommitWorkspaceMode = ({
	stackId,
	commitId,
}: RewordCommitWorkspaceMode): WorkspaceMode => ({
	_tag: "RewordCommit",
	stackId,
	commitId,
});

/** @public */
export const renameBranchWorkspaceMode = ({
	stackId,
	branchRef,
}: RenameBranchWorkspaceMode): WorkspaceMode => ({
	_tag: "RenameBranch",
	stackId,
	branchRef,
});

export const getOperationMode = (mode: WorkspaceMode): OperationMode | null =>
	Match.value(mode).pipe(
		Match.withReturnType<OperationMode | null>(),
		Match.tags({ Operation: ({ value }) => value }),
		Match.orElse(() => null),
	);

export const isAbsorptionPlanTargetItem = ({
	absorptionPlan,
	item,
}: {
	absorptionPlan: Array<CommitAbsorption>;
	item: Item;
}): boolean =>
	absorptionPlan.some((commitAbsorption) =>
		itemEquals(
			item,
			commitItem({
				stackId: commitAbsorption.stackId,
				commitId: commitAbsorption.commitId,
			}),
		),
	);

const operationModeToOperationType = (operationMode: OperationMode): OperationType | null =>
	Match.value(operationMode).pipe(
		Match.withReturnType<OperationType | null>(),
		Match.tags({
			Absorb: () => null,
			Rub: () => "rub",
			// We should have the ability to move either above or below.
			Move: ({ source }) => (source._tag === "Branch" ? "moveAbove" : "moveBelow"),
			DragAndDrop: ({ operationType }) => operationType,
		}),
		Match.exhaustive,
	);

export const operationModeToOperation = (
	operationMode: OperationMode,
	target: Item | null,
): Operation | null =>
	Match.value(operationMode).pipe(
		Match.withReturnType<Operation | null>(),
		Match.tags({ Absorb: absorbOperation }),
		Match.orElse((mode) => {
			if (!target) return null;

			return getOperation({
				source: mode.source,
				target,
				operationType: operationModeToOperationType(mode),
			});
		}),
	);

export const isValidWorkspaceMode = ({
	mode,
	navigationIndex,
}: {
	mode: WorkspaceMode;
	navigationIndex: NavigationIndex;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Operation: ({ value }) =>
				Match.value(value).pipe(
					Match.tagsExhaustive({
						Absorb: (mode) =>
							navigationIndexIncludes(navigationIndex, mode.source) &&
							mode.absorptionPlan.every((commitAbsorption) =>
								navigationIndexIncludes(
									navigationIndex,
									commitItem({
										stackId: commitAbsorption.stackId,
										commitId: commitAbsorption.commitId,
									}),
								),
							),
						Rub: (mode) => navigationIndexIncludes(navigationIndex, mode.source),
						Move: (mode) => navigationIndexIncludes(navigationIndex, mode.source),
						// Once we have keyboard selectable hunks, this should check the
						// navigation index(es).
						DragAndDrop: () => true,
					}),
				),
			RewordCommit: (mode) =>
				navigationIndexIncludes(
					navigationIndex,
					commitItem({
						stackId: mode.stackId,
						commitId: mode.commitId,
					}),
				),
			RenameBranch: (mode) =>
				navigationIndexIncludes(
					navigationIndex,
					branchItem({
						stackId: mode.stackId,
						branchRef: mode.branchRef,
					}),
				),
		}),
	);

export const isValidWorkspaceModeForSelectedItem = ({
	mode,
	selectedItem,
}: {
	mode: WorkspaceMode;
	selectedItem: Item;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Operation: () => true,
			RewordCommit: (mode) =>
				itemEquals(
					selectedItem,
					commitItem({
						stackId: mode.stackId,
						commitId: mode.commitId,
					}),
				),
			RenameBranch: (mode) =>
				itemEquals(
					selectedItem,
					branchItem({
						stackId: mode.stackId,
						branchRef: mode.branchRef,
					}),
				),
		}),
	);

export const includeItemForWorkspaceMode = ({
	mode,
	item,
}: {
	mode: WorkspaceMode;
	item: Item;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Operation: ({ value }) =>
				Match.value(value).pipe(
					Match.tagsExhaustive({
						Absorb: (mode) =>
							isAbsorptionPlanTargetItem({ absorptionPlan: mode.absorptionPlan, item }),
						DragAndDrop: ({ source }) => {
							const operations = getOperations(source, item);
							return !!operations.rub || !!operations.moveAbove || !!operations.moveBelow;
						},
						Move: (mode) =>
							!!getOperation({
								source: mode.source,
								target: item,
								operationType: operationModeToOperationType(mode),
							}),
						Rub: (mode) =>
							!!getOperation({
								source: mode.source,
								target: item,
								operationType: operationModeToOperationType(mode),
							}),
					}),
				),
			RenameBranch: (mode) =>
				itemEquals(
					item,
					branchItem({
						stackId: mode.stackId,
						branchRef: mode.branchRef,
					}),
				),
			RewordCommit: (mode) =>
				itemEquals(
					item,
					commitItem({
						stackId: mode.stackId,
						commitId: mode.commitId,
					}),
				),
		}),
	);
