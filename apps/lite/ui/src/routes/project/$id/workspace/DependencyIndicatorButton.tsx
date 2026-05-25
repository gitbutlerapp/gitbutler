import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getBranchNameByCommitId } from "#ui/api/ref-info.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { Tooltip } from "#ui/components/Tooltip.tsx";
import { useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Array, pipe } from "effect";
import { FC, useState } from "react";

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
		<Tooltip
			open={isTooltipOpen}
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
			trigger={
				<button
					{...restProps}
					type="button"
					onMouseEnter={highlightCommitIds}
					onMouseLeave={clearHighlightedCommitIds}
					onFocus={highlightCommitIds}
					onBlur={clearHighlightedCommitIds}
					aria-label={tooltip}
				/>
			}
			content={tooltip}
		/>
	);
};
