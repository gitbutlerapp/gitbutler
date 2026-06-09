import { Operand, operandEquals } from "#ui/operands.ts";
import { getOperationSource, pointerTransferOperationMode } from "#ui/outline/mode.ts";
import styles from "./OperationSourceC.module.css";
import { operationSourceLabel } from "./operationSourceLabel.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/components/classes.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { FC, type ReactNode, useEffect, useEffectEvent, useRef } from "react";
import { createRoot } from "react-dom/client";

type DragData = {
	source: Operand;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("source" in data)) return null;
	return data as DragData;
};

const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes(styles.dragPreview, "text-14")}>{children}</div>
);

export const OperationSourceC: FC<
	{
		onDragStart?: () => void;
		projectId: string;
		source: Operand;
	} & Omit<useRender.ComponentProps<"div">, "onDragStart">
> = ({ onDragStart: onDragStartProp, projectId, source, render, ...props }) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const dragRef = useRef<HTMLElement>(null);
	const onGenerateDragPreview: Parameters<typeof draggable>[0]["onGenerateDragPreview"] =
		useEffectEvent(({ nativeSetDragImage }) => {
			setCustomNativeDragPreview({
				nativeSetDragImage,
				getOffset: centerUnderPointer,
				render: ({ container }) => {
					if (!headInfo) return;
					const root = createRoot(container);
					root.render(<DragPreview>{operationSourceLabel({ source, headInfo })}</DragPreview>);
					return () => {
						root.unmount();
					};
				},
			});
		});
	const canDrag = useEffectEvent(
		() => outlineMode._tag !== "RenameBranch" && outlineMode._tag !== "RewordCommit",
	);
	const onDragStart = useEffectEvent(() => {
		onDragStartProp?.();
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: pointerTransferOperationMode({
					source,
					operationType: null,
				}),
			}),
		);
	});
	const getInitialData = useEffectEvent((): DragData => ({ source }));

	useEffect(() => {
		const element = dragRef.current;
		if (!element) return;

		return draggable({
			element,
			// Prevent false positives when users drag to select text in the input field.
			canDrag,
			getInitialData,
			onGenerateDragPreview,
			onDragStart,
			onDrop: ({ location }) => {
				if (location.current.dropTargets.length > 0) return;

				dispatch(projectActions.cancelMode({ projectId }));
			},
		});
	}, [dispatch, projectId]);

	const operationSource = getOperationSource(outlineMode);
	const isActiveSource = operationSource ? operandEquals(operationSource, source) : false;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActiveSource && styles.activeSource),
		}),
	});
};
