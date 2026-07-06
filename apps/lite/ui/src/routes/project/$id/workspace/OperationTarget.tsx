import { operandEquals, type Operand } from "#ui/operands.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { parseDragData } from "./DragData.ts";
import styles from "./OperationTarget.module.css";
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
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC, useEffect, useEffectEvent, useRef } from "react";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useQuery } from "@tanstack/react-query";

type DropTargetParams = Parameters<typeof dropTargetForElements>[0];
type GetDataArgs = Parameters<NonNullable<DropTargetParams["getData"]>>[0];
type OnDropArgs = Parameters<NonNullable<DropTargetParams["onDrop"]>>[0];

type DropData = OnDropArgs["self"]["data"];

const getOperationTypeFromData = (data: DropData): OperationType | null => {
	const instruction = extractInstruction(data);
	if (!instruction) return null;

	return Match.value(instruction.operation).pipe(
		Match.withReturnType<OperationType>(),
		Match.when("combine", () => "into"),
		Match.when("reorder-before", () => "above"),
		Match.when("reorder-after", () => "below"),
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
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const getData = useEffectEvent(({ input, element, source }: GetDataArgs) => {
		const dragData = parseDragData(source.data);
		if (!dragData) return {};

		const { into, above, below } = getOperations(dragData.source, target, { headInfoIndex });
		return attachInstruction(
			{},
			{
				input,
				element,
				operations: {
					"reorder-before": above ? "available" : "not-available",
					"reorder-after": below ? "available" : "not-available",
					combine: into ? "available" : "not-available",
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
								headInfoIndex,
							})
						: null;

				if (!operation) {
					dispatch(projectActions.cancelMode({ projectId }));
					return;
				}

				dispatch(projectActions.exitMode({ projectId }));
				runOperation(operation.operation);
			},
		});
	}, [dispatch, headInfoIndex, projectId, runOperation, target]);

	return { dropRef, headInfoIndex };
};

export type OperationTargetOutline = "inside" | "outside";

export const OperationTarget: FC<
	{
		enabled: boolean;
		target: Operand;
		projectId: string;
		isSelected: boolean;
		isAbsorptionTarget: boolean;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ enabled, target, projectId, isSelected, isAbsorptionTarget, outline, render, ...props }) => {
	const { dropRef, headInfoIndex } = useOperationDropTarget({ enabled, target, projectId });

	const activeTargetOperationType = useAppSelector((state) => {
		const outlineMode = selectProjectOutlineModeState(state, projectId);

		return Match.value(outlineMode).pipe(
			Match.withReturnType<OperationType | null>(),
			Match.when({ _tag: "Absorb" }, () => (isAbsorptionTarget ? "into" : null)),
			Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) =>
				mode.target &&
				operandEquals(mode.target, target) &&
				(mode.operationType !== "into" || !operandEquals(mode.source, target))
					? mode.operationType
					: null,
			),
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: mode }) =>
				isSelected && (mode.operationType !== "into" || !operandEquals(mode.source, target))
					? mode.operationType
					: null,
			),
			Match.orElse(() => null),
		);
	});

	const targetEl = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(
				activeTargetOperationType === "into" &&
					classes(
						styles.activeTarget,
						Match.value(outline).pipe(
							Match.when("inside", () => styles.activeTargetInside),
							Match.when("outside", () => styles.activeTargetOutside),
							Match.exhaustive,
						),
					),
			),
		}),
	});

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const tooltip =
		activeTargetOperationType !== null
			? Match.value(outlineMode).pipe(
					Match.when({ _tag: "Absorb" }, () => <>Absorb target</>),
					Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) =>
						mode.target && mode.operationType !== null
							? getOperation({
									source: mode.source,
									target: mode.target,
									operationType: mode.operationType,
									headInfoIndex,
								})?.label
							: undefined,
					),
					Match.when(
						{ _tag: "Transfer", value: { _tag: "Keyboard" } },
						({ value: mode }) =>
							getOperation({
								source: mode.source,
								target,
								operationType: mode.operationType,
								headInfoIndex,
							})?.label,
					),
					Match.orElse(() => undefined),
				)
			: undefined;

	return (
		<Tooltip.Root open={tooltip !== undefined} disableHoverablePopup>
			<Tooltip.Trigger
				render={targetEl}
				className={pipe(
					activeTargetOperationType,
					Match.value,
					Match.when("above", () => classes(styles.insertionTarget, styles.insertionTargetAbove)),
					Match.when("below", () => classes(styles.insertionTarget, styles.insertionTargetBelow)),
					Match.whenOr("into", null, () => undefined),
					Match.exhaustive,
				)}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
