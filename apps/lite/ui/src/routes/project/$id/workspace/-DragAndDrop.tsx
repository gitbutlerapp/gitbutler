import { classes } from "#ui/classes.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { type Operation, useRunOperation } from "#ui/Operation.ts";
import { CommitLabel, formatHunkHeader, Patch } from "#ui/routes/project/$id/-shared.tsx";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { mergeProps, useRender } from "@base-ui/react";
import { Commit, DiffHunk, HunkAssignment, TreeChange } from "@gitbutler/but-sdk";
import { FC, ReactNode, useEffect } from "react";
import { type SourceItem } from "./-SourceItem.ts";
import styles from "./route.module.css";

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

const DragPreview: FC<{
	children: ReactNode;
}> = ({ children }) => <div className={styles.dragPreview}>{children}</div>;

export const DraggableBranch: FC<
	{
		anchorRef: Array<number> | null;
		label: string;
	} & useRender.ComponentProps<"div">
> = ({ anchorRef, label, render, ...props }) => {
	const dragData: DragData | null =
		anchorRef !== null ? { sourceItem: { _tag: "Branch", anchorRef } } : null;
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData | {} => dragData ?? {},
		preview: <DragPreview>{label}</DragPreview>,
		canDrag: () => dragData !== null,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

export const DraggableCommit: FC<
	{
		commit: Commit;
		canDrag?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ commit, canDrag = true, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: { _tag: "Commit", commitId: commit.id },
		}),
		preview: (
			<DragPreview>
				<CommitLabel commit={commit} />
			</DragPreview>
		),
		canDrag: () => canDrag,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

export const DraggableFile: FC<
	{
		change: TreeChange;
		changeUnit: ChangeUnit;
		assignments?: Array<HunkAssignment>;
	} & useRender.ComponentProps<"div">
> = ({ change, changeUnit, assignments, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: {
				_tag: "TreeChange",
				source: {
					parent: changeUnit,
					change,
					hunkHeaders: assignments
						? assignments.flatMap((assignment) =>
								assignment.hunkHeader != null ? [assignment.hunkHeader] : [],
							)
						: [],
				},
			},
		}),
		preview: <DragPreview>{change.path}</DragPreview>,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

export const DraggableHunk: FC<
	{
		patch: Patch;
		changeUnit: ChangeUnit;
		change: TreeChange;
		hunk: DiffHunk;
	} & useRender.ComponentProps<"div">
> = ({ patch, changeUnit, change, hunk, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: {
				_tag: "TreeChange",
				source: {
					parent: changeUnit,
					change,
					hunkHeaders: [hunk],
				},
			},
		}),
		preview: <DragPreview>Hunk {formatHunkHeader(hunk)}</DragPreview>,
		canDrag: () => !patch.subject.isResultOfBinaryToTextConversion,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
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
