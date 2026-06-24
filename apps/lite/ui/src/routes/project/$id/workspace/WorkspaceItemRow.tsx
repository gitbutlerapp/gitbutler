import { classes } from "#ui/components/classes.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, MouseEvent, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";
import { mergeProps, useRender } from "@base-ui/react";
import { Match } from "effect";

const isFromInteractiveDescendant = (event: MouseEvent<HTMLDivElement>): boolean => {
	if (!(event.target instanceof Element)) return false;
	const interactiveElement = event.target.closest(["button", "input[type='checkbox']"].join(","));
	return interactiveElement !== null && event.currentTarget.contains(interactiveElement);
};

export const WorkspaceItemRow: FC<
	{
		isSelected?: boolean;
		onSelect?: () => void;
		/** @default false */
		isHighlighted?: boolean;
		interactive?: boolean;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ isSelected, onSelect, isHighlighted, interactive = true, ref: refProp, ...props }) => {
	const rowRef = useRef<HTMLDivElement | null>(null);
	const mergedRef = useMergedRefs(rowRef, refProp);

	useLayoutEffect(() => {
		if (!isSelected) return;
		rowRef.current?.scrollIntoView({
			block: "nearest",
			inline: "nearest",
		});
	}, [isSelected]);

	return (
		// This is safe because the tree is focusable.
		// oxlint-disable-next-line jsx_a11y/click-events-have-key-events, jsx_a11y/no-static-element-interactions
		<div
			{...props}
			ref={mergedRef}
			className={classes(
				props.className,
				styles.container,
				isSelected && styles.containerSelected,
				isHighlighted && styles.containerHighlighted,
				interactive && styles.containerInteractive,
			)}
			onMouseDown={(event) => {
				props.onMouseDown?.(event);

				if (
					!event.defaultPrevented &&
					// Prevent clicks on interactive descendants from stealing focus from the tree.
					isFromInteractiveDescendant(event)
				)
					event.preventDefault();
			}}
			onClick={(event) => {
				props.onClick?.(event);

				if (
					!event.defaultPrevented &&
					// Prevent clicks on interactive descendants from stealing selection.
					!isFromInteractiveDescendant(event)
				)
					onSelect?.();
			}}
		/>
	);
};

export const WorkspaceItemRowLabelContainer: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.labelContainer)} />
);

export const WorkspaceItemRowLabel: FC<{ heading?: boolean } & useRender.ComponentProps<"div">> = ({
	heading,
	render,
	...props
}) =>
	useRender({
		render,
		props: mergeProps<"div">(props, {
			className: classes(styles.label, heading ? "text-14" : "text-13", heading && "text-bold"),
		}),
	});

type WorkspaceItemRowBubbleVariant = "fillGray" | "safe" | "danger";

export const WorkspaceItemRowBubble: FC<
	{
		variant: WorkspaceItemRowBubbleVariant;
	} & ComponentProps<"span">
> = ({ variant, ...props }) => (
	<span
		{...props}
		className={classes(
			props.className,
			"text-11",
			"text-semibold",
			styles.bubble,
			Match.value(variant).pipe(
				Match.when("fillGray", () => styles.bubbleFillGray),
				Match.when("safe", () => styles.bubbleClrSafe),
				Match.when("danger", () => styles.bubbleClrDanger),
				Match.exhaustive,
			),
		)}
	/>
);

export const WorkspaceItemRowBubbleGroup: FC<ComponentProps<"span">> = (props) => (
	<span {...props} className={classes(props.className, styles.bubbleGroup)} />
);

export const WorkspaceItemRowToolbar: FC<{ forceVisible?: boolean } & ComponentProps<"div">> = ({
	forceVisible,
	...props
}) => (
	<div
		{...props}
		className={classes(props.className, styles.toolbar, forceVisible && styles.toolbarForceVisible)}
	/>
);
