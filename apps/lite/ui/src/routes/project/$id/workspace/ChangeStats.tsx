import { Badge } from "#ui/components/Badge.tsx";
import { classes } from "#ui/components/classes.ts";
import type { FC } from "react";
import styles from "./ChangeStats.module.css";
import type { LineStats } from "./lineStats.ts";

/**
 * File count and added/removed line totals for a set of changes.
 *
 * Shown in the files panel header, and in the diff toolbar when that panel is hidden.
 */
export const ChangeStats: FC<{
	fileCount: number;
	lineStats: LineStats;
	className?: string;
}> = ({ fileCount, lineStats, className }) => (
	<span className={classes(styles.container, className)}>
		<Badge variant="fillGray">{fileCount}</Badge>

		{(lineStats.linesAdded > 0 || lineStats.linesRemoved > 0) && (
			<span className={classes("text-12", styles.lineStats)}>
				{lineStats.linesAdded > 0 && (
					<span className={styles.linesAdded}>+{lineStats.linesAdded}</span>
				)}
				{lineStats.linesRemoved > 0 && (
					<span className={styles.linesRemoved}>-{lineStats.linesRemoved}</span>
				)}
			</span>
		)}
	</span>
);
