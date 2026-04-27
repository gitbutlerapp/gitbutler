import { type Item } from "./Item.ts";
import { parseDragData } from "./OperationSourceC.tsx";
import styles from "./OperationTarget.module.css";
import { OperationTooltip } from "./OperationTooltip.tsx";
import { getOperation, getOperations, OperationType, useRunOperation } from "#ui/Operation.ts";
import { classes } from "#ui/classes.ts";
import {
	projectActions,
	selectProjectOperationModeState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { isAbsorptionPlanTargetItem } from "#ui/routes/project/$id/workspace/WorkspaceMode.ts";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC, useEffect, useEffectEvent, useRef, useState } from "react";

type DropTargetParams = Parameters<typeof dropTargetForElements>[0];
type GetDataArgs = Parameters<NonNullable<DropTargetParams["getData"]>>[0];

type DropData = {
	operationType: OperationType;
	target: Item;
};

const parseDropData = (data: unknown): DropData | null => {
	if (typeof data !== "object" || data === null || !("operationType" in data)) return null;
	return data as DropData;
};

const getDropOperationType = ({
	source,
	target,
	input,
	element,
}: {
	source: Item;
	target: Item;
	input: Parameters<typeof attachInstruction>[1]["input"];
	element: Parameters<typeof attachInstruction>[1]["element"];
}): OperationType | null => {
	const { rub, moveAbove, moveBelow } = getOperations(source, target);

	const instruction = extractInstruction(
		attachInstruction(
			{},
			{
				input,
				element,
				operations: {
					"reorder-before": moveAbove ? "available" : "not-available",
					"reorder-after": moveBelow ? "available" : "not-available",
					combine: rub ? "available" : "not-available",
				},
			},
		),
	);
	if (!instruction) return null;

	return Match.value(instruction.operation).pipe(
		Match.withReturnType<OperationType | null>(),
		Match.when("combine", () => "rub"),
		Match.when("reorder-before", () => "moveAbove"),
		Match.when("reorder-after", () => "moveBelow"),
		Match.exhaustive,
	);
};

const useOperationDropTarget = ({ item, projectId }: { item: Item; projectId: string }) => {
	const dispatch = useAppDispatch();
	const runOperation = useRunOperation();
	const dropRef = useRef<HTMLElement>(null);
	const [isActiveDropTarget, setIsActiveDropTarget] = useState<boolean>(false);

	const getDropData = useEffectEvent(({ input, element, source }: GetDataArgs): DropData | null => {
		const dragData = parseDragData(source.data);
		if (!dragData) return null;

		const operationType = getDropOperationType({
			source: dragData.source,
			target: item,
			input,
			element,
		});
		if (operationType === null) return null;

		return { operationType, target: item };
	});

	useEffect(() => {
		const element = dropRef.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			getData: (args) => getDropData(args) ?? {},
			canDrop: (args) => getDropData(args) !== null,
			onDrag: (args) => {
				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				setIsActiveDropTarget(isActiveDropTarget);

				if (!isActiveDropTarget) return;

				const dropData = parseDropData(args.self.data);

				dispatch(
					projectActions.updateDragAndDropMode({
						projectId,
						operationType: dropData?.operationType ?? null,
					}),
				);
			},
			onDragLeave: () => {
				setIsActiveDropTarget(false);
			},
			onDrop: (args) => {
				setIsActiveDropTarget(false);

				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				if (!isActiveDropTarget) return;

				const dragData = parseDragData(args.source.data);
				if (!dragData) return;

				const dropData = parseDropData(args.self.data);
				if (!dropData) return;

				const operation = getOperation({
					source: dragData.source,
					target: dropData.target,
					operationType: dropData.operationType,
				});
				if (!operation) return;

				runOperation(projectId, operation);
			},
		});
	}, [dispatch, projectId, runOperation]);

	return { dropRef, isActiveDropTarget };
};

export const OperationTarget: FC<
	{
		item: Item;
		projectId: string;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ item, projectId, isSelected, render, ...props }) => {
	const { dropRef, isActiveDropTarget } = useOperationDropTarget({ item, projectId });
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const insertTargetOperationType = operationMode
		? Match.value(operationMode).pipe(
				Match.tagsExhaustive({
					Absorb: () => null,
					DragAndDrop: ({ operationType }) =>
						isActiveDropTarget && (operationType === "moveAbove" || operationType === "moveBelow")
							? operationType
							: null,
					Rub: () => null,
					Move: () => null,
				}),
			)
		: null;

	const isMainTargetActive =
		!!operationMode &&
		Match.value(operationMode).pipe(
			Match.tagsExhaustive({
				Absorb: ({ absorptionPlan }) => isAbsorptionPlanTargetItem({ absorptionPlan, item }),
				DragAndDrop: ({ operationType }) => isActiveDropTarget && operationType === "rub",
				Rub: () => isSelected,
				Move: () => isSelected,
			}),
		);

	const isMainTargetTooltipActive =
		!!operationMode &&
		Match.value(operationMode).pipe(
			Match.tagsExhaustive({
				Absorb: () => isSelected,
				DragAndDrop: ({ operationType }) => isActiveDropTarget && operationType === "rub",
				Rub: () => isSelected,
				Move: () => isSelected,
			}),
		);

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(isMainTargetActive && styles.activeTarget),
		}),
	});

	return (
		<div className={styles.target}>
			<OperationTooltip
				projectId={projectId}
				item={item}
				isActive={isMainTargetTooltipActive}
				operationMode={operationMode}
				render={target}
			/>

			{insertTargetOperationType !== null && (
				<OperationTooltip
					projectId={projectId}
					item={item}
					isActive
					operationMode={operationMode}
					className={classes(
						styles.insertionTarget,
						pipe(
							insertTargetOperationType,
							Match.value,
							Match.when("moveAbove", () => styles.insertionTargetAbove),
							Match.when("moveBelow", () => styles.insertionTargetBelow),
							Match.exhaustive,
						),
					)}
				/>
			)}
		</div>
	);
};
