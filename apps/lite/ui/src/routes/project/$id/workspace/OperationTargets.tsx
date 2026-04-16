import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { classes } from "#ui/classes.ts";
import { changesSectionFileParent, commitFileParent } from "#ui/domain/FileParent.ts";
import { type GetDataParams, useDroppable } from "#ui/hooks/useDroppable.ts";
import { getInsertionSide, useRunOperation, type Operation } from "#ui/Operation.ts";
import { projectActions } from "#ui/routes/project/$id/state/projectSlice.ts";
import { useAppDispatch } from "#ui/state/hooks.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC } from "react";
import { DropData, parseDragData } from "./DragAndDrop.tsx";
import { type Item } from "./Item.ts";
import { operationModeToOperation } from "./OperationMode.tsx";
import { OperationTooltip } from "./OperationTooltip.tsx";
import {
	getBranchTargetOperation,
	getCombineOperation,
	getCommitTargetMoveOperation,
	getTearOffBranchTargetOperation,
	useResolveOperationSource,
	type ResolvedOperationSource,
} from "./ResolvedOperationSource.ts";
import { type OperationMode } from "./WorkspaceMode.ts";
import styles from "./route.module.css";

const useDragOperation = ({
	projectId,
	getOperation,
}: {
	projectId: string;
	getOperation: (
		args: GetDataParams[0] & { resolvedOperationSource: ResolvedOperationSource },
	) => Operation | null;
}) => {
	const resolveOperationSource = useResolveOperationSource(projectId);

	return useDroppable((args): DropData => {
		const operationSource = parseDragData(args.source.data);
		if (!operationSource) return null;

		const resolvedOperationSource = resolveOperationSource(operationSource);
		if (!resolvedOperationSource) return null;

		return { operationSource, operation: getOperation({ ...args, resolvedOperationSource }) };
	});
};

const useOperationTarget = ({
	projectId,
	item,
	operationMode,
	isSelected,
	getOperation,
}: {
	projectId: string;
	item: Item;
	operationMode: OperationMode | null;
	isSelected: boolean;
	getOperation: (
		args: GetDataParams[0] & { resolvedOperationSource: ResolvedOperationSource },
	) => Operation | null;
}) => {
	const dispatch = useAppDispatch();
	const runOperation = useRunOperation();

	const [drag, dropRef] = useDragOperation({ projectId, getOperation });

	const resolveOperationSource = useResolveOperationSource(projectId);
	const resolvedOperationModeSource = operationMode
		? resolveOperationSource(operationMode.source)
		: null;
	const isActiveOperationModeTarget = !!operationMode && isSelected;
	const operationModeOperation =
		isActiveOperationModeTarget && resolvedOperationModeSource
			? operationModeToOperation({
					operationMode,
					resolvedOperationSource: resolvedOperationModeSource,
					target: item,
				})
			: null;

	const confirmMode = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operationModeOperation) return;

		runOperation(projectId, operationModeOperation);
	};

	const cancelMode = () => dispatch(projectActions.exitMode({ projectId }));

	return {
		drag,
		dropRef,
		isActiveTarget: !!drag?.operation || isActiveOperationModeTarget,
		source: drag?.operationSource ?? operationMode?.source,
		operation: drag?.operation ?? operationModeOperation,
		controls: isActiveOperationModeTarget
			? { onConfirm: confirmMode, onCancel: cancelMode }
			: undefined,
	};
};

const dropTargetToOperation = ({
	target,
	resolvedOperationSource,
}: {
	target: Item;
	resolvedOperationSource: ResolvedOperationSource;
}) =>
	Match.value(target).pipe(
		Match.tags({
			ChangesSection: () =>
				getCombineOperation({
					resolvedOperationSource,
					target: changesSectionFileParent({}),
				}),
			Branch: ({ branchRef }) =>
				getBranchTargetOperation({
					resolvedOperationSource,
					branchRef,
				}),
			Commit: ({ commitId }) =>
				getCombineOperation({
					resolvedOperationSource,
					target: commitFileParent({ commitId }),
				}),
			BaseCommit: () => getTearOffBranchTargetOperation(resolvedOperationSource),
		}),
		Match.orElse(() => null),
	);

export const OperationTarget: FC<
	{
		projectId: string;
		item: Item;
		operationMode: OperationMode | null;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, item, operationMode, isSelected, render, ...props }) => {
	const { dropRef, isActiveTarget, source, operation, controls } = useOperationTarget({
		projectId,
		item,
		operationMode,
		isSelected,
		getOperation: ({ resolvedOperationSource }) =>
			dropTargetToOperation({ target: item, resolvedOperationSource }),
	});

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(isActiveTarget && styles.activeTarget),
		}),
	});

	return (
		<OperationTooltip
			controls={controls}
			enabled={isActiveTarget}
			sourceItem={item}
			operation={operation ?? null}
			render={target}
			sourceOperation={source}
		/>
	);
};

const commitDropTargetToOperation = ({
	input,
	element,
	resolvedOperationSource,
	commitId,
}: GetDataParams[0] & {
	resolvedOperationSource: ResolvedOperationSource;
	commitId: string;
}) => {
	const combine = getCombineOperation({
		resolvedOperationSource,
		target: commitFileParent({ commitId }),
	});
	const insertAbove = getCommitTargetMoveOperation({
		resolvedOperationSource,
		commitId,
		side: "above",
	});
	const insertBelow = getCommitTargetMoveOperation({
		resolvedOperationSource,
		commitId,
		side: "below",
	});

	const instruction = extractInstruction(
		attachInstruction(
			{ resolvedOperationSource },
			{
				input,
				element,
				operations: {
					"reorder-before": insertAbove ? "available" : "not-available",
					"reorder-after": insertBelow ? "available" : "not-available",
					combine: combine ? "available" : "not-available",
				},
			},
		),
	);

	if (!instruction) return null;

	return Match.value(instruction.operation).pipe(
		Match.when("combine", () => combine),
		Match.when("reorder-before", () => insertAbove),
		Match.when("reorder-after", () => insertBelow),
		Match.exhaustive,
	);
};

export const CommitTarget: FC<
	{
		commitId: string;
		item: Item;
		projectId: string;
		operationMode: OperationMode | null;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ commitId, item, projectId, operationMode, isSelected, render, ...props }) => {
	const { drag, dropRef, isActiveTarget, source, operation, controls } = useOperationTarget({
		projectId,
		item,
		operationMode,
		isSelected,
		getOperation: (args) => commitDropTargetToOperation({ ...args, commitId }),
	});

	const dragInsertionSide = drag?.operation ? getInsertionSide(drag.operation) : null;

	const targetTooltipOperation = dragInsertionSide === null ? operation : null;

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(targetTooltipOperation && styles.activeTarget),
		}),
	});

	return (
		<div className={styles.commit}>
			<OperationTooltip
				controls={controls}
				enabled={isActiveTarget}
				sourceItem={item}
				operation={targetTooltipOperation ?? null}
				render={target}
				sourceOperation={source}
			/>

			{drag && dragInsertionSide !== null && (
				<OperationTooltip
					controls={controls}
					enabled={!!drag.operation}
					sourceItem={item}
					operation={drag.operation}
					sourceOperation={drag.operationSource}
					className={classes(
						styles.commitInsertionTarget,
						pipe(
							dragInsertionSide,
							Match.value,
							Match.when("above", () => styles.commitInsertionTargetAbove),
							Match.when("below", () => styles.commitInsertionTargetBelow),
							Match.exhaustive,
						),
					)}
				/>
			)}
		</div>
	);
};
