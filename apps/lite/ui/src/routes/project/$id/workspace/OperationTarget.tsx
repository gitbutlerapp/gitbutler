import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { classes } from "#ui/classes.ts";
import { getInsertionSide, moveOperation, rubOperation, type Operation } from "#ui/Operation.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC } from "react";
import { type GetDataParams, useDroppable } from "./DragAndDrop.tsx";
import { parseDragData } from "./OperationDragAndDrop.tsx";
import { type Item } from "./Item.ts";
import { operationModeToOperation } from "./OperationMode.tsx";
import { OperationTooltip } from "./OperationTooltip.tsx";
import { type OperationMode } from "./WorkspaceMode.ts";
import styles from "./OperationTarget.module.css";

const dropTargetToOperation =
	(target: Item, source: Item) =>
	({ input, element }: GetDataParams[0]): Operation | null => {
		const combine = rubOperation({
			source,
			target,
		});
		const insertAbove = moveOperation({
			source,
			target,
			side: "above",
		});
		const insertBelow = moveOperation({
			source,
			target,
			side: "below",
		});

		const instruction = extractInstruction(
			attachInstruction(
				{},
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

export type TargetData = {
	source: Item;
	operation: Operation | null;
};

const useDropTarget = ({ item }: { projectId: string; item: Item }) =>
	useDroppable((args): TargetData | null => {
		const dragData = parseDragData(args.source.data);
		if (!dragData) return null;

		const { source } = dragData;
		const operation = dropTargetToOperation(item, source)(args);

		return {
			source,
			operation,
		};
	});

const useOperationModeTarget = ({
	item,
	operationMode,
	isSelected,
}: {
	projectId: string;
	item: Item;
	operationMode: OperationMode | null;
	isSelected: boolean;
}): TargetData | null => {
	const isActiveTarget = !!operationMode && isSelected;

	if (!isActiveTarget) return null;

	const { source } = operationMode;
	const operation = operationModeToOperation({
		operationMode,
		source,
		target: item,
	});

	return {
		source,
		operation,
	};
};

export const OperationTarget: FC<
	{
		item: Item;
		projectId: string;
		operationMode: OperationMode | null;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ item, projectId, operationMode, isSelected, render, ...props }) => {
	const [dropData, dropRef] = useDropTarget({ projectId, item });
	const operationModeTarget = useOperationModeTarget({
		projectId,
		item,
		operationMode,
		isSelected,
	});

	const targetData: TargetData | null = dropData ?? operationModeTarget;

	const dropInsertionSide = dropData?.operation ? getInsertionSide(dropData.operation) : null;

	const mainTargetData = dropInsertionSide === null ? targetData : null;

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(mainTargetData && styles.activeTarget),
		}),
	});

	return (
		<div className={styles.target}>
			<OperationTooltip
				projectId={projectId}
				isOperationMode={!!operationMode}
				item={item}
				operation={mainTargetData?.operation ?? null}
				source={mainTargetData?.source}
				render={target}
			/>

			{dropInsertionSide !== null && (
				<OperationTooltip
					projectId={projectId}
					isOperationMode={false}
					item={item}
					operation={dropData?.operation ?? null}
					source={dropData?.source}
					className={classes(
						styles.insertionTarget,
						pipe(
							dropInsertionSide,
							Match.value,
							Match.when("above", () => styles.insertionTargetAbove),
							Match.when("below", () => styles.insertionTargetBelow),
							Match.exhaustive,
						),
					)}
				/>
			)}
		</div>
	);
};
