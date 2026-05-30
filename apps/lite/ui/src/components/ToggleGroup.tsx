import { classes } from "#ui/components/classes.ts";
import styles from "./ToggleGroup.module.css";
import { ComponentProps } from "react";

export function ToggleGroupStyles(props: ComponentProps<"div">) {
	return <div {...props} className={classes(props.className, styles.group)} />;
}

export function ToggleStyles(props: ComponentProps<"button">) {
	return (
		<button {...props} type="button" className={classes(props.className, "text-13", styles.item)} />
	);
}
