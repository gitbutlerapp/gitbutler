import { FC } from "react";
import { classes } from "#ui/components/classes.ts";
import styles from "./DiffStat.module.css";

interface Props {
	linesAdded: number;
	linesRemoved: number;
}

export const DiffStat: FC<Props> = ({ linesAdded, linesRemoved }) => (
	<div className={styles.container}>
		<span className={classes("text-11", styles.added)}>+{linesAdded}</span>
		<span className={classes("text-11", styles.removed)}>-{linesRemoved}</span>
	</div>
);
