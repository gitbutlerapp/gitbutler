import { classes } from "./classes.ts";
import { getButtonClassName, type ButtonSize } from "./Button.tsx";
import styles from "./ToggleGroup.module.css";
import type { ComponentProps, FC } from "react";

/**
 * @public
 * @import import { ToggleGroupStyles } from "@gitbutler/ui-react/ToggleGroup.tsx";
 */
export const ToggleGroupStyles: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.group)} />
);

/** @public */
export const ToggleStyles: FC<
	ComponentProps<"button"> & { iconOnly?: boolean; size?: ButtonSize }
> = ({ iconOnly, size, ...props }) => (
	<button
		{...props}
		type="button"
		className={classes(props.className, getButtonClassName({ iconOnly, size }), styles.item)}
	/>
);
