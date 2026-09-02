import { Badge } from "#ui/components/Badge.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { workspaceStacksSummary } from "#ui/segment.ts";
import { Tooltip } from "@base-ui/react";
import type { Stack } from "@gitbutler/but-sdk";
import type { FC } from "react";
import styles from "./StacksSummary.module.css";

const pluralRules = new Intl.PluralRules("en");
const branches = (count: number) =>
	`${count} branch${pluralRules.select(count) === "one" ? "" : "es"}`;

/**
 * Stands in for the stacks a folded panel hides, the way a folded branch row's
 * count stands in for its commits.
 *
 * The branch count says how much is down there; the unpushed count and the
 * conflict marker are the parts that can change while the panel is shut, and
 * they are the reason this is worth showing at all — each appears only when it
 * has something to report, so seeing one always means something.
 */
export const StacksSummary: FC<{ stacks: Array<Stack> }> = ({ stacks }) => {
	const { branches: branchCount, unpushedBranches, hasConflicts } = workspaceStacksSummary(stacks);
	if (branchCount === 0) return null;

	const spoken = [
		branches(branchCount),
		...(unpushedBranches > 0 ? [`${unpushedBranches} with unpushed commits`] : []),
		...(hasConflicts ? ["some conflicted"] : []),
	].join(", ");

	return (
		<Tooltip.Root>
			<Tooltip.Trigger render={<span aria-label={spoken} className={styles.container} />}>
				<Badge variant="lightGray">{branchCount}</Badge>

				{unpushedBranches > 0 && (
					<span className={classes("text-12", styles.unpushed)}>
						{unpushedBranches}
						<Icon size={12} name="arrow-up" />
					</span>
				)}

				{hasConflicts && (
					<Badge variant="warn">
						<Icon size={12} name="warning" />
					</Badge>
				)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{spoken}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
