import { useRunOperation } from "#ui/Operation.ts";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { useEffect } from "react";
import { TargetData } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { Item } from "./Item";

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

export const useMonitorDraggedItem = ({ projectId }: { projectId: string }) => {
	const runOperation = useRunOperation();

	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDrop: ({ location }) => {
					const dropData = location.current.dropTargets
						.map((dropTarget) => parseDropData(dropTarget.data))
						.find((dropData) => dropData?.operation);

					if (!dropData?.operation) return;

					runOperation(projectId, dropData.operation);
				},
			}),
		[runOperation, projectId],
	);
};
