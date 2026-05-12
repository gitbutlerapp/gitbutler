import { Operand, operandEquals } from "#ui/operands.ts";
import styles from "./OperationSourceC.module.css";
import { OperationSourceLabel } from "./OperationSourceLabel.tsx";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/ui/classes.ts";
import {
	projectActions,
	selectProjectOperationModeState,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { mergeProps, useRender } from "@base-ui/react";
import { useSuspenseQuery } from "@tanstack/react-query";
import { FC, type ReactNode, useEffect, useEffectEvent, useRef } from "react";
import { createRoot } from "react-dom/client";

type DragData = {
	source: Operand;
};

export const parseDragData = (data: unknown): DragData | undefined => {
	if (typeof data !== "object" || data === null || !("source" in data)) return undefined;
	return data as DragData;
};

const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.dragPreview}>{children}</div>
);

export const OperationSourceC: FC<
	{
		projectId: string;
		source: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, source, render, ...props }) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

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
	const canDrag = useEffectEvent(
		() => outlineMode._tag !== "RenameBranch" && outlineMode._tag !== "RewordCommit",
	);

	useEffect(() => {
		const element = dragRef.current;
		if (!element) return;

		return draggable({
			element,
			// Prevent false positives when users drag to select text in the input field.
			canDrag,
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

	const isActiveSource = operationMode?.source && operandEquals(operationMode.source, source);

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActiveSource && styles.activeSource),
		}),
	});
};
