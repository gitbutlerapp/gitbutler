import { type Operand } from "#ui/operands.ts";
import { parseDragData } from "./DragData.ts";
import styles from "./OperationTarget.module.css";
import {
	getOperation,
	getOperations,
	type OperationType,
	useRunOperation,
} from "#ui/operations/operation.ts";
import { classes } from "#ui/components/classes.ts";
import { OutlineModeContext } from "#ui/WorkspaceContext.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { Tooltip, useRender } from "@base-ui/react";
import { Match } from "effect";
import { FC, use, useEffect, useEffectEvent, useRef } from "react";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";

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

const useOperationDropTarget = ({ enabled, target }: { enabled: boolean; target: Operand }) => {
	const { updatePointerTransfer, cancelMode, exitMode } = use(OutlineModeContext);
	const { mutate: runOperation } = useRunOperation();
	const dropRef = useRef<HTMLElement>(null);

	const getData = useEffectEvent(({ input, element, source }: GetDataArgs) => {
		const dragData = parseDragData(source.data);
		if (!dragData) return {};

		const { into, above, below } = getOperations(dragData.source, target);
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

				updatePointerTransfer(target, operationType);
			},
			onDragLeave: () => {
				updatePointerTransfer(null, null);
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
					cancelMode();
					return;
				}

				exitMode();
				runOperation(operation.operation);
			},
		});
	}, [cancelMode, exitMode, runOperation, target, updatePointerTransfer]);

	return { dropRef };
};

export type OperationTargetOutline = "inside" | "outside";

export type ActiveOperation = { operationType: OperationType; tooltip?: string | undefined };

export const OperationTarget: FC<
	{
		enabled: boolean;
		target: Operand;
		activeOperation?: ActiveOperation | null;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ enabled, target, activeOperation, outline, render, ...props }) => {
	const { dropRef } = useOperationDropTarget({ enabled, target });

	const targetEl = useRender({
		render,
		ref: dropRef,
		props,
	});

	return (
		<Tooltip.Root open={activeOperation?.tooltip !== undefined} disableHoverablePopup>
			<Tooltip.Trigger
				render={targetEl}
				className={Match.value(activeOperation?.operationType).pipe(
					Match.when("above", () => classes(styles.insertionTarget, styles.insertionTargetAbove)),
					Match.when("below", () => classes(styles.insertionTarget, styles.insertionTargetBelow)),
					Match.when("into", () =>
						classes(
							styles.activeTarget,
							Match.value(outline).pipe(
								Match.when("inside", () => styles.activeTargetInside),
								Match.when("outside", () => styles.activeTargetOutside),
								Match.exhaustive,
							),
						),
					),
					Match.when(undefined, () => undefined),
					Match.exhaustive,
				)}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{activeOperation?.tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
