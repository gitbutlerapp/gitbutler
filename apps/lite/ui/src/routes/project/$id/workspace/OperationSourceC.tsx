import { Operand, operandEquals } from "#ui/operands.ts";
import { getOperationSource, pointerTransferMode } from "#ui/outline/mode.ts";
import styles from "./OperationSourceC.module.css";
import { operandLabel } from "./operandLabel.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { classes } from "#ui/components/classes.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { createContext, FC, type ReactNode, use, useEffect, useEffectEvent, useRef } from "react";
import { createRoot } from "react-dom/client";
import type { DragData } from "./DragData.ts";
import { Match } from "effect";

const DragPreview: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes(styles.dragPreview, "text-13")}>{children}</div>
);

type OperationSourceContextValue = {
	projectId: string;
	headInfoIndex: ReturnType<typeof getHeadInfoIndex> | undefined;
	outlineMode: ReturnType<typeof selectProjectOutlineModeState>;
	dispatch: ReturnType<typeof useAppDispatch>;
};

const OperationSourceContext = createContext<OperationSourceContextValue | null>(null);

type OperationSourceOutline = "inside" | "outside";

type OperationSourceCProps = {
	projectId: string;
	source: Operand;
	outline: OperationSourceOutline;
} & Omit<useRender.ComponentProps<"div">, "onDragStart">;

export const OperationSourceProvider: FC<{ projectId: string; children: ReactNode }> = ({
	projectId,
	children,
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();

	return (
		<OperationSourceContext value={{ projectId, headInfoIndex, outlineMode, dispatch }}>
			{children}
		</OperationSourceContext>
	);
};

export const OperationSourceC: FC<OperationSourceCProps> = (props) => {
	const context = use(OperationSourceContext);

	if (context?.projectId === props.projectId)
		return <OperationSourceCInner {...props} context={context} />;

	return <StandaloneOperationSourceC {...props} />;
};

const StandaloneOperationSourceC: FC<OperationSourceCProps> = (props) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(props.projectId),
		select: getHeadInfoIndex,
	});
	const outlineMode = useAppSelector((state) =>
		selectProjectOutlineModeState(state, props.projectId),
	);
	const dispatch = useAppDispatch();

	return (
		<OperationSourceCInner
			{...props}
			context={{ projectId: props.projectId, headInfoIndex, outlineMode, dispatch }}
		/>
	);
};

const OperationSourceCInner: FC<
	OperationSourceCProps & { context: OperationSourceContextValue }
> = ({ projectId, source, outline, render, context, ...props }) => {
	const { dispatch, headInfoIndex, outlineMode } = context;
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
						<DragPreview>{operandLabel({ operand: source, headInfoIndex })}</DragPreview>,
					);
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
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: pointerTransferMode({
					source,
					target: null,
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
