import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getBranchNameByCommitId } from "#ui/api/ref-info.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Array, pipe } from "effect";
import { ComponentProps, FC } from "react";

export const DependencyIndicatorButton: FC<
	{
		projectId: string;
		commitIds: Array.NonEmptyArray<string>;
	} & ComponentProps<"button">
> = ({ projectId, commitIds, ...restProps }) => {
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
		dispatch(
			projectActions.setHighlightedCommitIds({
				projectId,
				commitIds,
			}),
		);
	};
	const clearHighlightedCommitIds = () => {
		dispatch(projectActions.setHighlightedCommitIds({ projectId, commitIds: null }));
	};

	return (
		<Tooltip.Root
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger
				{...restProps}
				onMouseEnter={highlightCommitIds}
				onMouseLeave={clearHighlightedCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightedCommitIds}
				aria-label={tooltip}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup content={tooltip} />} />
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
