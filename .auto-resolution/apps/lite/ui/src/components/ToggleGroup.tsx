import { classes } from "#ui/components/classes.ts";
import { getButtonClassName } from "./Button.tsx";
import styles from "./ToggleGroup.module.css";
import { ComponentProps, FC } from "react";

/** @public */
export const ToggleGroupStyles: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.group)} />
);

/** @public */
export const ToggleStyles: FC<ComponentProps<"button"> & { iconOnly?: boolean }> = ({
	iconOnly,
	...props
}) => (
	<button
		{...props}
		type="button"
		className={classes(props.className, getButtonClassName({ iconOnly }), styles.item)}
	/>
);
