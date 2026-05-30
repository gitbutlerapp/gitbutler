import { classes } from "#ui/components/classes.ts";
import styles from "./ToggleGroup.module.css";
import { ComponentProps, FC } from "react";

export const ToggleGroupStyles: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.group)} />
);

export const ToggleStyles: FC<ComponentProps<"button">> = (props) => (
	<button {...props} type="button" className={classes(props.className, "text-13", styles.item)} />
);
