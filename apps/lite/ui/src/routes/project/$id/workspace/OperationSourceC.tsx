import { type Address, addressIdentityKey } from "#ui/addresses.ts";
import { cancelPendingOperation } from "#ui/use-cursor.ts";
import { getOperationSources, pointerTransfer } from "#ui/operations/pending-operation.ts";
import styles from "./OperationSourceC.module.css";
import { addressesLabel } from "./addressLabel.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { classes } from "#ui/components/classes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import {
	draggable,
	dropTargetForElements,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
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
		sources: Array<Address>;
		/** Whether any checked source expands the transfer to the complete checked set. */
		respectChecked: boolean;
		outline: OperationSourceOutline;
		/**
		 * Accept dropping a drag back on its exact source element when it has no organic operation
		 * target. Do not enable this when the element already mounts an `OperationTarget`, as Pragmatic
		 * DnD only supports one element drop-target registration per element.
		 */
		acceptOriginDrop?: boolean;
	} & Omit<useRender.ComponentProps<"div">, "onDragStart">
> = ({
	projectId,
	sources,
	respectChecked,
	outline,
	acceptOriginDrop = false,
	render,
	...props
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const pendingOperation = useAppSelector((state) =>
		projectSlice.selectors.selectPendingOperation(state, projectId),
	);
	const store = useAppStore();

	const resolveDragSources = (): Array<Address> => {
		if (!respectChecked) return sources;

		const state = store.getState();
		const checkedAddressKeys = projectSlice.selectors.selectCheckedAddressKeys(state, projectId);
		return sources.some((source) => checkedAddressKeys.has(addressIdentityKey(source)))
			? projectSlice.selectors.selectCheckedAddresses(state, projectId)
			: sources;
	};

	const dispatch = useAppDispatch();
	const dragRef = useRef<HTMLElement>(null);
	const onGenerateDragPreview: Parameters<typeof draggable>[0]["onGenerateDragPreview"] =
		useEffectEvent(({ nativeSetDragImage }) => {
			const dragSources = resolveDragSources();

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
		const dragSources = resolveDragSources();

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

	const getInitialData = useEffectEvent((): DragData => ({ sources: resolveDragSources() }));

	useEffect(() => {
		const element = dragRef.current;
		if (!element) return;

		const cleanupDraggable = draggable({
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
		const cleanupOriginDropTarget = acceptOriginDrop
			? dropTargetForElements({
					element,
					canDrop: ({ source }) => source.element === element,
					onDrop: cancelPendingOperation,
				})
			: undefined;

		return () => {
			cleanupDraggable();
			cleanupOriginDropTarget?.();
		};
	}, [acceptOriginDrop, dispatch, projectId]);

	const operationSources = getOperationSources(pendingOperation);
	const operationSourceKeys = operationSources && new Set(operationSources.map(addressIdentityKey));
	const isActiveSource = operationSourceKeys
		? sources.every((source) => operationSourceKeys.has(addressIdentityKey(source)))
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
