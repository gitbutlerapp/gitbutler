import { Item, itemEquals } from "./Item.ts";
import styles from "./OperationSourceC.module.css";
import { OperationSourceLabel } from "./OperationSourceLabel.tsx";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import {
	projectActions,
	selectProjectOperationModeState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { mergeProps, useRender } from "@base-ui/react";
import { useSuspenseQuery } from "@tanstack/react-query";
import { FC, type ReactNode, useEffect, useEffectEvent, useRef } from "react";
import { createRoot } from "react-dom/client";

type DragData = {
	source: Item;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("source" in data)) return null;
	return data as DragData;
};

const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.dragPreview}>{children}</div>
);

export const OperationSourceC: FC<
	{
		projectId: string;
		source: Item;
	} & useRender.ComponentProps<"div">
> = ({ projectId, source, render, ...props }) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const dispatch = useAppDispatch();
	const dragRef = useRef<HTMLElement>(null);
	const onGenerateDragPreview = useEffectEvent(
		({ nativeSetDragImage }: { nativeSetDragImage: DataTransfer["setDragImage"] | null }) => {
			setCustomNativeDragPreview({
				nativeSetDragImage,
				getOffset: centerUnderPointer,
				render: ({ container }) => {
					const root = createRoot(container);
					root.render(
						<DragPreview>
							<OperationSourceLabel source={source} headInfo={headInfo} />
						</DragPreview>,
					);
					return () => {
						root.unmount();
					};
				},
			});
		},
	);

	useEffect(() => {
		const element = dragRef.current;
		if (!element) return;

		return draggable({
			element,
			getInitialData: (): DragData => ({ source }),
			onGenerateDragPreview,
			onDragStart: () => {
				dispatch(projectActions.enterDragAndDropMode({ projectId, source }));
			},
			onDrop: () => {
				dispatch(projectActions.exitMode({ projectId }));
			},
		});
	}, [dispatch, projectId, source]);

	const isActiveSource = operationMode?.source && itemEquals(operationMode.source, source);

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActiveSource && styles.activeSource),
		}),
	});
};
