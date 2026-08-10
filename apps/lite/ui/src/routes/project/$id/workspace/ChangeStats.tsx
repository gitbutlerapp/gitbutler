import { Badge } from "#ui/components/Badge.tsx";
import { classes } from "#ui/components/classes.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import type { FC } from "react";
import styles from "./ChangeStats.module.css";
import type { LineStats } from "./lineStats.ts";

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
	const spoken = [description];
	if (lineStats.linesAdded > 0) {
		spoken.push(
			`${lineStats.linesAdded} line${pluralRules.select(lineStats.linesAdded) === "one" ? "" : "s"} added`,
		);
	}
	if (lineStats.linesRemoved > 0) {
		spoken.push(
			`${lineStats.linesRemoved} line${pluralRules.select(lineStats.linesRemoved) === "one" ? "" : "s"} removed`,
		);
	}

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<span aria-label={spoken.join(", ")} className={classes(styles.container, className)}>
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
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{description}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
