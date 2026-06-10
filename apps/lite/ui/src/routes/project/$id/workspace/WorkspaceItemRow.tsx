import { classes } from "#ui/components/classes.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";
import { mergeProps, useRender } from "@base-ui/react";

export const WorkspaceSection: FC<useRender.ComponentProps<"div">> = ({ render, ...props }) =>
	useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			className: styles.section,
		}),
	});

export const WorkspaceItemRow: FC<
	{
		isSelected?: boolean;
		onSelect?: () => void;
		/** @default false */
		isHighlighted?: boolean;
		heading?: boolean;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ isSelected, onSelect, isHighlighted, heading, ref: refProp, ...props }) => {
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
				heading ? "text-14" : "text-13",
				heading && "text-bold",
				styles.itemRow,
				heading && styles.itemRowHeading,
				isSelected && styles.itemRowSelected,
				isHighlighted && styles.itemRowHighlighted,
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

export const WorkspaceItemRowEmpty: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, "text-13", styles.itemRowEmpty)} />
);

export const WorkspaceItemRowLabel: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.itemRowLabel)} />
);

export const WorkspaceItemRowToolbar: FC<{ forceVisible?: boolean } & ComponentProps<"div">> = ({
	forceVisible,
	...props
}) => (
	<div
		{...props}
		className={classes(
			props.className,
			styles.itemRowToolbar,
			forceVisible && styles.itemRowToolbarForceVisible,
		)}
	/>
);

export const WorkspaceItemRowIconButton: FC<
	{ isSelected: boolean } & useRender.ComponentProps<"button">
> = ({ isSelected, render, ...props }) =>
	useRender({
		render,
		defaultTagName: "button",
		props: mergeProps<"button">(props, {
			className: classes(
				styles.itemRowIconButton,
				getButtonClassName({
					variant: isSelected ? "inverted" : "ghost",
					size: "small",
				}),
			),
		}),
	});
