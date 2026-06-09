import { classes } from "#ui/components/classes.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";

export const WorkspaceItemRow: FC<
	{
		isSelected?: boolean;
		onSelect?: () => void;
		/** @default false */
		isHighlighted?: boolean;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ isSelected, onSelect, isHighlighted, ref: refProp, ...props }) => {
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
				"text-13",
				styles.itemRow,
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

export const WorkspaceItemRowToolbar: FC<
	{ forceVisibleToolbar?: boolean } & ComponentProps<"div">
> = ({ forceVisibleToolbar, ...props }) => (
	<div
		{...props}
		className={classes(
			props.className,
			styles.itemRowToolbar,
			forceVisibleToolbar && styles.forceVisibleToolbar,
		)}
	/>
);
