import { classes } from "./classes.ts";
import { getButtonClassName, type ButtonSize } from "./Button.tsx";
import styles from "./ToggleGroup.module.css";
import type { ComponentProps, FC } from "react";

/**
 * A row of toggles sharing one border, for picking a view or a mode. In a single-select group the
 * pressed item can't be un-pressed, so it doesn't answer to hover; a group whose items switch off
 * on their own keeps its hover.
 *
 * @public
 * @import import { ToggleGroupStyles } from "@gitbutler/but-ui-react/ToggleGroup.tsx";
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
