import { type Address, addressEquals } from "#ui/addresses.ts";
import { cancelPendingOperation } from "#ui/use-cursor.ts";
import { getOperationSources, pointerTransfer } from "#ui/operations/pending-operation.ts";
import styles from "./OperationSourceC.module.css";
import { addressesLabel } from "./addressLabel.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { classes } from "#ui/components/classes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type FC, type ReactNode, useEffect, useEffectEvent, useRef } from "react";
import { createRoot } from "react-dom/client";
import type { DragData } from "./DragData.ts";
import { Match } from "effect";

export const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes(styles.dragPreview, "text-13")}>{children}</div>
);

type OperationSourceOutline = "inside" | "outside";

export const OperationSourceC: FC<
	{
		projectId: string;
		source: Address;
		outline: OperationSourceOutline;
	} & Omit<useRender.ComponentProps<"div">, "onDragStart">
> = ({ projectId, source, outline, render, ...props }) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const pendingOperation = useAppSelector((state) =>
		projectSlice.selectors.selectPendingOperation(state, projectId),
	);
	// We don't necessarily wrap in an array here in order to preserve reference identity.
	const dragSource = useAppSelector((state) => {
		if (source._tag !== "Commit" && source._tag !== "File") return source;

		const isChecked = projectSlice.selectors.selectAddressChecked(state, projectId, source);
		return isChecked ? projectSlice.selectors.selectCheckedAddresses(state, projectId) : source;
	});
	const dragSources = Array.isArray(dragSource) ? dragSource : [dragSource];

	const dispatch = useAppDispatch();
	const dragRef = useRef<HTMLElement>(null);
	const onGenerateDragPreview: Parameters<typeof draggable>[0]["onGenerateDragPreview"] =
		useEffectEvent(({ nativeSetDragImage }) => {
			setCustomNativeDragPreview({
				nativeSetDragImage,
				getOffset: centerUnderPointer,
				render: ({ container }) => {
					if (!headInfoIndex) return;
					const root = createRoot(container);
					root.render(
						<DragPreview>{addressesLabel({ addresses: dragSources, headInfoIndex })}</DragPreview>,
					);
					return () => {
						root.unmount();
					};
				},
			});
		});
	const canDrag = useEffectEvent(() => pendingOperation._tag !== "InlineEdit");
	const onDragStart = useEffectEvent(() => {
		dispatch(
			projectSlice.actions.startTransfer({
				projectId,
				transfer: pointerTransfer({
					sources: dragSources,
					target: null,
					placement: null,
				}),
			}),
		);
	});
	const getInitialData = useEffectEvent((): DragData => ({ sources: dragSources }));

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

				cancelPendingOperation();
			},
		});
	}, [dispatch, projectId]);

	const operationSources = getOperationSources(pendingOperation);
	const isActiveSource = operationSources
		? operationSources.some((operationSource) => addressEquals(operationSource, source))
		: false;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(
				isActiveSource &&
					classes(
						styles.activeSource,
						Match.value(outline).pipe(
							Match.when("inside", () => styles.activeSourceInside),
							Match.when("outside", () => styles.activeSourceOutside),
							Match.exhaustive,
						),
					),
			),
		}),
	});
};
