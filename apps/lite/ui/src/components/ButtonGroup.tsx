import styles from "./ButtonGroup.module.css";
import { classes } from "#ui/components/classes.ts";
import { type ComponentProps, type FC } from "react";

type Props = ComponentProps<"div">;

export const ButtonGroup: FC<Props> = ({ className, role = "group", ...props }) => (
	<div {...props} role={role} className={classes(styles.group, className)} />
);
