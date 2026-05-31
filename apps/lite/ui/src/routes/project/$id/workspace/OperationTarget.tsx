import { type Operand } from "#ui/operands.ts";
import { parseDragData } from "./OperationSourceC.tsx";
import styles from "./OperationTarget.module.css";
import { OperationTooltip } from "./OperationTooltip.tsx";
import {
	getOperation,
	getOperations,
	type OperationType,
	useRunOperation,
} from "#ui/operations/operation.ts";
import { classes } from "#ui/components/classes.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC, useEffect, useEffectEvent, useRef } from "react";

type DropTargetParams = Parameters<typeof dropTargetForElements>[0];
type GetDataArgs = Parameters<NonNullable<DropTargetParams["getData"]>>[0];
type OnDropArgs = Parameters<NonNullable<DropTargetParams["onDrop"]>>[0];

type DropData = OnDropArgs["self"]["data"];

const getOperationTypeFromData = (data: DropData): OperationType | null => {
	const instruction = extractInstruction(data);
	if (!instruction) return null;

	return Match.value(instruction.operation).pipe(
		Match.withReturnType<OperationType>(),
		Match.when("combine", () => "rub"),
		Match.when("reorder-before", () => "moveAbove"),
		Match.when("reorder-after", () => "moveBelow"),
		Match.exhaustive,
	);
};

const useOperationDropTarget = ({
	enabled,
	target,
	projectId,
}: {
	enabled: boolean;
	target: Operand;
	projectId: string;
}) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useRunOperation();
	const dropRef = useRef<HTMLElement>(null);

	const getData = useEffectEvent(({ input, element, source }: GetDataArgs) => {
		const dragData = parseDragData(source.data);
		if (!dragData) return {};

		const { rub, moveAbove, moveBelow } = getOperations(dragData.source, target);
		return attachInstruction(
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
		);
	});

	const canDrop = useEffectEvent(() => enabled);

	useEffect(() => {
		const element = dropRef.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			getData,
			canDrop,
			onDrag: (args) => {
				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				if (!isActiveDropTarget) return;

				const operationType = getOperationTypeFromData(args.self.data);

				dispatch(
					projectActions.updatePointerTransfer({
						projectId,
						target,
						operationType,
					}),
				);
			},
			onDragLeave: () => {
				dispatch(
					projectActions.updatePointerTransfer({
						projectId,
						target: null,
						operationType: null,
					}),
				);
			},
			onDrop: (args) => {
				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				if (!isActiveDropTarget) return;

				const dragData = parseDragData(args.source.data);
				const operationType = getOperationTypeFromData(args.self.data);
				const operation =
					dragData && operationType !== null
						? getOperation({
								source: dragData.source,
								target,
								operationType,
							})
						: null;

				if (!operation) {
					dispatch(projectActions.cancelMode({ projectId }));
					return;
				}

				dispatch(projectActions.exitMode({ projectId }));
				runOperation(operation);
			},
		});
	}, [dispatch, projectId, runOperation, target]);

	return { dropRef };
};

export const OperationTarget: FC<
	{
		enabled: boolean;
		target: Operand;
		projectId: string;
		isSelected: boolean;
		isAbsorptionTarget: boolean;
	} & useRender.ComponentProps<"div">
> = ({ enabled, target, projectId, isSelected, isAbsorptionTarget, render, ...props }) => {
	const { dropRef } = useOperationDropTarget({ enabled, target, projectId });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const insertTargetOperationType = Match.value(outlineMode).pipe(
		Match.tag("Transfer", ({ value: mode }) =>
			isSelected && (mode.operationType === "moveAbove" || mode.operationType === "moveBelow")
				? mode.operationType
				: null,
		),
		Match.orElse(() => null),
	);

	const isMainTargetActive = Match.value(outlineMode).pipe(
		Match.tags({
			Absorb: () => isAbsorptionTarget,
			Transfer: ({ value: mode }) => isSelected && mode.operationType === "rub",
		}),
		Match.orElse(() => false),
	);

	const isMainTargetTooltipActive = Match.value(outlineMode).pipe(
		Match.tags({
			Absorb: () => isSelected,
			Transfer: () => isMainTargetActive,
		}),
		Match.orElse(() => false),
	);

	const targetEl = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(isMainTargetActive && styles.activeTarget),
		}),
	});

	return (
		<div className={styles.target}>
			<OperationTooltip
				target={target}
				isActive={isMainTargetTooltipActive}
				outlineMode={outlineMode}
				render={targetEl}
			/>

			{insertTargetOperationType !== null && (
				<OperationTooltip
					target={target}
					isActive
					outlineMode={outlineMode}
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
