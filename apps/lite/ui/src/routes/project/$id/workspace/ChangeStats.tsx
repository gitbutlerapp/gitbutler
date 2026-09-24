import { Badge } from "@gitbutler/ui-react/Badge.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { DiffStats } from "@gitbutler/ui-react/DiffStats.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import type { FC } from "react";
import styles from "./ChangeStats.module.css";
import { describeLineStats, type LineStats } from "./lineStats.ts";

const pluralRules = new Intl.PluralRules("en");

/**
 * File count and added/removed line totals for a set of changes.
 *
 * Shown in the files panel header, and in the diff toolbar when that panel is hidden.
 */
export const ChangeStats: FC<{
	fileCount: number;
	lineStats: LineStats;
	className?: string;
}> = ({ fileCount, lineStats, className }) => {
	// The file count is the only genuinely ambiguous number — a green +N next to a red -N reads
	// as added/removed lines on sight — so the tooltip only explains that one. Screen readers
	// get the full wording instead, since the colours carry no meaning for them.
	const description = `${fileCount} file${pluralRules.select(fileCount) === "one" ? "" : "s"} changed`;
	const spoken = [description, ...describeLineStats(lineStats)];

	return (
		<Tooltip content={description}>
			<span aria-label={spoken.join(", ")} className={classes(styles.container, className)}>
				<Badge variant="lightGray">{fileCount}</Badge>

				<DiffStats
					added={lineStats.linesAdded}
					removed={lineStats.linesRemoved}
					className="text-12"
				/>
			</span>
		</Tooltip>
	);
};
