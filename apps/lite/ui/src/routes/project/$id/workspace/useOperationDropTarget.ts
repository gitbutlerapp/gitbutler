import {
	getOperation,
	getOperations,
	type OperationType,
	useRunOperation,
} from "#ui/operations/operation.ts";
import { type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { Match } from "effect";
import { useEffect, useEffectEvent, useRef } from "react";
import { parseDragData } from "./DragData.ts";
import { useQuery } from "@tanstack/react-query";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";

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

export const useOperationDropTarget = ({
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
		if (!headInfoIndex || !dragData) return {};

		const { into, above, below } = getOperations(dragData.sources, target, headInfoIndex);
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
					projectSlice.actions.updatePointerTransfer({
						projectId,
						target,
						operationType,
					}),
				);
			},
			onDragLeave: () => {
				dispatch(
					projectSlice.actions.updatePointerTransfer({
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
					headInfoIndex && dragData && operationType !== null
						? getOperation({
								sources: dragData.sources,
								target,
								operationType,
								headInfoIndex,
							})
						: null;

				if (!operation) {
					dispatch(projectSlice.actions.cancelMode({ projectId }));
					return;
				}

				dispatch(projectSlice.actions.exitMode({ projectId }));
				runOperation(operation.operation);
			},
		});
	}, [dispatch, projectId, runOperation, target, headInfoIndex]);

	return dropRef;
};
