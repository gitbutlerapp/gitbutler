import { classes } from "#ui/components/classes.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ComponentProps, FC, useLayoutEffect, useRef } from "react";
import styles from "./WorkspaceItemRow.module.css";
import { getButtonClassName } from "#ui/components/Button.tsx";

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

export const WorkspaceItemRowLabel: FC<{ heading?: boolean } & ComponentProps<"div">> = ({
	heading,
	...restProps
}) => (
	<div
		{...restProps}
		className={classes(
			restProps.className,
			styles.itemRowLabel,
			heading ? "text-14" : "text-13",
			heading && "text-bold",
			heading ? styles.itemRowLabelHeading : styles.itemRowLabelRegular,
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

export const getWorkspaceItemRowButtonClassName = ({ iconOnly = false }: { iconOnly?: boolean }) =>
	classes(
		getButtonClassName({
			variant: "ghost",
			size: "small",
			iconOnly,
			// On selection/focus change we change the button variant. This
			// transition would clash with other selection/focus style changes
			// which are instant (e.g. the row background).
			disableTransition: true,
		}),
		styles.itemRowButton,
	);
