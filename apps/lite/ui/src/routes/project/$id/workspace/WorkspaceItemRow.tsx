import { classes } from "#ui/ui/classes.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";

export const WorkspaceItemRow: FC<
	{
		isSelected?: boolean;
		onSelect?: () => void;
		/** @default false */
		forceVisibleToolbar?: boolean;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ className, isSelected, forceVisibleToolbar, onSelect, ref: refProp, ...props }) => {
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
				className,
				styles.itemRow,
				isSelected && styles.itemRowSelected,
				forceVisibleToolbar && styles.forceVisibleToolbar,
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

export const WorkspaceItemRowToolbar: FC<
	Omit<ComponentProps<typeof Toolbar.Root>, "className">
> = ({ onClick, ...props }) => (
	<Toolbar.Root
		{...props}
		className={styles.itemRowToolbar}
		onClick={(event) => {
			onClick?.(event);
			event.stopPropagation();
		}}
	/>
);
