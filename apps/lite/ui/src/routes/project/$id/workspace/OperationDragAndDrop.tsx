import { useRunOperation } from "#ui/Operation.ts";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { useEffect, useState } from "react";
import { TargetData } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { Item } from "./Item";
import { type Operation } from "#ui/Operation.ts";

export type DragData = {
	source: Item;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("source" in data)) return null;
	return data as DragData;
};

const parseDropData = (data: unknown): TargetData | null => {
	if (typeof data !== "object" || data === null || !("operation" in data)) return null;
	return data as TargetData;
};

const getOperationFromDropTargets = (dropTargets: Array<{ data: unknown }>): Operation | null =>
	dropTargets
		.map((dropTarget) => parseDropData(dropTarget.data))
		.find((dropData) => dropData?.operation)?.operation ?? null;

export const useMonitorDraggedItem = ({ projectId }: { projectId: string }): Operation | null => {
	const runOperation = useRunOperation();
	const [operation, setOperation] = useState<Operation | null>(null);

	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDrag: ({ location }) => {
					setOperation(getOperationFromDropTargets(location.current.dropTargets));
				},
				onDrop: ({ location }) => {
					const operation = getOperationFromDropTargets(location.current.dropTargets);
					setOperation(null);

					if (!operation) return;

					runOperation(projectId, operation);
				},
			}),
		[runOperation, projectId],
	);

	return operation;
};
