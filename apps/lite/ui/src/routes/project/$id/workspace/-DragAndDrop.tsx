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
import { getCombineOperation, type SourceItem } from "./-SourceItem.ts";

type DragData = {
	sourceItem: SourceItem;
};

export const parseDragData = (data: unknown): SourceItem | null => {
	if (typeof data !== "object" || data === null || !("sourceItem" in data)) return null;
	return (data as DragData).sourceItem;
};

const parseDropTargetData = (data: unknown): Operation | null => {
	if (typeof data !== "object" || data === null || !("_tag" in data)) return null;
	return data as Operation;
};

export const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={sharedStyles.dragPreview}>{children}</div>
);

export const getDragData = (sourceItem: SourceItem | null): DragData | null =>
	sourceItem !== null ? { sourceItem } : null;

export const getCommitTargetInstruction = ({
	sourceItem,
	commitId,
	previousCommitId,
	nextCommitId,
	input,
	element,
}: {
	sourceItem: SourceItem;
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

	const getSourceCommitId = (item: SourceItem): string | null =>
		item._tag === "Commit"
			? item.commitId
			: item._tag === "TreeChanges" && item.parent._tag === "Commit"
				? item.parent.commitId
				: null;

	const combineOperation = getCombineOperation({
		sourceItem,
		target: { _tag: "Commit", commitId },
	});

	return extractInstruction(
		attachInstruction(
			{ sourceItem },
			{
				input,
				element,
				operations: {
					"reorder-before":
						(sourceItem._tag === "Commit" && !isNoOpCommitMove(sourceItem.commitId, "above")) ||
						(sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Changes") ||
						(sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Commit")
							? "available"
							: "not-available",
					"reorder-after":
						(sourceItem._tag === "Commit" && !isNoOpCommitMove(sourceItem.commitId, "below")) ||
						(sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Changes") ||
						(sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Commit")
							? "available"
							: "not-available",
					combine:
						combineOperation || getSourceCommitId(sourceItem) === commitId
							? "available"
							: "not-available",
				},
			},
		),
	);
};

export const useMonitorDraggedSourceItem = ({ projectId }: { projectId: string }) => {
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
