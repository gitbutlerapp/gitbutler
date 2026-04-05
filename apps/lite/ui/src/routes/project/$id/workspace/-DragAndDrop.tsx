import { type Operation, useRunOperation } from "#ui/Operation.ts";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
	Instruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { type InsertSide } from "@gitbutler/but-sdk";
import { FC, type ReactNode, useEffect } from "react";
import sharedStyles from "../-shared.module.css";
import { getCombineOperation, type OperationSource } from "./-OperationSource.ts";

type DragData = {
	operationSource: OperationSource;
};

export const parseDragData = (data: unknown): OperationSource | null => {
	if (typeof data !== "object" || data === null || !("operationSource" in data)) return null;
	return (data as DragData).operationSource;
};

const parseDropTargetData = (data: unknown): Operation | null => {
	if (typeof data !== "object" || data === null || !("_tag" in data)) return null;
	return data as Operation;
};

export const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={sharedStyles.dragPreview}>{children}</div>
);

export const getDragData = (operationSource: OperationSource): DragData => ({ operationSource });

export const getCommitTargetInstruction = ({
	operationSource,
	commitId,
	previousCommitId,
	nextCommitId,
	input,
	element,
}: {
	operationSource: OperationSource;
	commitId: string;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
	input: Parameters<typeof attachInstruction>[1]["input"];
	element: Element;
}): Instruction | null => {
	const isNoOpCommitMove = (sourceCommitId: string, side: InsertSide): boolean =>
		sourceCommitId === commitId ||
		(side === "above" && previousCommitId === sourceCommitId) ||
		(side === "below" && nextCommitId === sourceCommitId);

	const getSourceCommitId = (item: OperationSource): string | null =>
		item._tag === "Commit"
			? item.commitId
			: item._tag === "TreeChanges" && item.parent._tag === "Commit"
				? item.parent.commitId
				: null;

	const combineOperation = getCombineOperation({
		operationSource,
		target: { _tag: "Commit", commitId },
	});

	return extractInstruction(
		attachInstruction(
			{ operationSource },
			{
				input,
				element,
				operations: {
					"reorder-before":
						(operationSource._tag === "Commit" &&
							!isNoOpCommitMove(operationSource.commitId, "above")) ||
						(operationSource._tag === "TreeChanges" && operationSource.parent._tag === "Changes") ||
						(operationSource._tag === "TreeChanges" && operationSource.parent._tag === "Commit")
							? "available"
							: "not-available",
					"reorder-after":
						(operationSource._tag === "Commit" &&
							!isNoOpCommitMove(operationSource.commitId, "below")) ||
						(operationSource._tag === "TreeChanges" && operationSource.parent._tag === "Changes") ||
						(operationSource._tag === "TreeChanges" && operationSource.parent._tag === "Commit")
							? "available"
							: "not-available",
					combine:
						combineOperation || getSourceCommitId(operationSource) === commitId
							? "available"
							: "not-available",
				},
			},
		),
	);
};

export const useMonitorDraggedOperationSource = ({ projectId }: { projectId: string }) => {
	const runOperation = useRunOperation(projectId);

	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDrop: ({ location }) => {
					const operation = location.current.dropTargets
						.map((dropTarget) => parseDropTargetData(dropTarget.data))
						.find((target) => target);

					if (!operation) return;

					runOperation(operation);
				},
			}),
		[runOperation],
	);
};
