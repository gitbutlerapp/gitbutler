import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { useSuspenseQuery } from "@tanstack/react-query";
import { FC, type ReactNode } from "react";
import { useDraggable } from "./DragAndDrop.tsx";
import { type DragData } from "./OperationDragAndDrop.tsx";
import { OperationSourceLabel } from "./OperationSourceLabel.tsx";
import { type OperationMode } from "./WorkspaceMode.ts";
import styles from "./OperationSourceC.module.css";
import { Item, itemEquals } from "./Item.ts";

const DragPreview = ({ children }: { children: ReactNode }) => (
	<div className={styles.dragPreview}>{children}</div>
);

export const OperationSourceC: FC<
	{
		operationMode?: OperationMode | null;
		projectId: string;
		source: Item;
		canDrag?: () => boolean;
	} & useRender.ComponentProps<"div">
> = ({ operationMode = null, projectId, source, canDrag, render, ...props }) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({ source }),
		preview: (
			<DragPreview>
				<OperationSourceLabel source={source} headInfo={headInfo} />
			</DragPreview>
		),
		canDrag,
	});

	const isActiveOperationModeSource =
		operationMode?.source && itemEquals(operationMode.source, source);

	const isActive = isDragging || isActiveOperationModeSource;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};
