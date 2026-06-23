import { classes } from "#ui/components/classes.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";
import { mergeProps, useRender } from "@base-ui/react";

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
			onClick={(event) => {
				props.onClick?.(event);
				if (!event.defaultPrevented) onSelect?.();
			}}
			onContextMenu={(event) => {
				props.onContextMenu?.(event);
				onSelect?.();
			}}
		/>
	);
};

export const WorkspaceItemRowLabelContainer: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.labelContainer)} />
);

export const WorkspaceItemRowLabel: FC<
	{ heading?: boolean; singleLine?: boolean } & useRender.ComponentProps<"div">
> = ({ heading, singleLine, render, ...props }) =>
	useRender({
		render,
		props: mergeProps<"div">(props, {
			className: classes(
				styles.label,
				singleLine && styles.labelSingleLine,
				heading ? "text-14" : "text-13",
				heading && "text-bold",
			),
		}),
	});

export const WorkspaceItemRowToolbar: FC<{ forceVisible?: boolean } & ComponentProps<"div">> = ({
	forceVisible,
	...props
}) => (
	<div
		{...props}
		className={classes(props.className, styles.toolbar, forceVisible && styles.toolbarForceVisible)}
	/>
);
