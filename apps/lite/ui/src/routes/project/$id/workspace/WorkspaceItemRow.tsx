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
				styles.itemRow,
				isSelected && styles.itemRowSelected,
				isHighlighted && styles.itemRowHighlighted,
				interactive && styles.itemRowInteractive,
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

export const WorkspaceItemRowLabel: FC<
	{ heading?: boolean; empty?: boolean } & ComponentProps<"div">
> = ({ heading, empty, ...restProps }) => (
	<div
		{...restProps}
		className={classes(
			restProps.className,
			styles.itemRowLabel,
			heading ? "text-14" : "text-13",
			heading && "text-bold",
			heading ? styles.itemRowLabelHeading : styles.itemRowLabelRegular,
			empty && styles.itemRowLabelEmpty,
		)}
	/>
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
