import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getBranchNameByCommitId } from "#ui/api/ref-info.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";
import { Tooltip, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Array, pipe } from "effect";
import { FC, useState } from "react";
import styles from "./DependencyIndicatorButton.module.css";

export const DependencyIndicatorButton: FC<
	{
		projectId: string;
		commitIds: Array.NonEmptyArray<string>;
	} & useRender.ComponentProps<"button">
> = ({ projectId, commitIds, ...restProps }) => {
	// We use a controlled tooltip as a workaround for https://github.com/mui/base-ui/issues/4499.
	const [isTooltipOpen, setIsTooltipOpen] = useState(false);
	const dispatch = useAppDispatch();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	// TODO: expensive
	const branchNameByCommitId = headInfo
		? getBranchNameByCommitId(headInfo)
		: new Map<string, string>();
	const branchNames = pipe(
		commitIds,
		Array.flatMapNullable((commitId) => branchNameByCommitId.get(commitId)),
		Array.dedupe,
	);
	const tooltip =
		branchNames.length > 0 ? `Depends on ${branchNames.join(", ")}` : "Unknown dependencies";
	const highlightCommitIds = () => {
		setIsTooltipOpen(true);
		dispatch(
			projectActions.setHighlightedCommitIds({
				projectId,
				commitIds,
			}),
		);
	};
	const clearHighlightedCommitIds = () => {
		setIsTooltipOpen(false);
		dispatch(projectActions.setHighlightedCommitIds({ projectId, commitIds: null }));
	};

	return (
		<Tooltip.Root
			open={isTooltipOpen}
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger
				{...restProps}
				type="button"
				onMouseEnter={highlightCommitIds}
				onMouseLeave={clearHighlightedCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightedCommitIds}
				aria-label={tooltip}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.popup)}>
						{tooltip}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
