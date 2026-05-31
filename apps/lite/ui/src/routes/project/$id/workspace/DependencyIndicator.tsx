import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getBranchNameByCommitId } from "#ui/api/ref-info.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Popover } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Array, pipe } from "effect";
import { ComponentProps, FC } from "react";

export const DependencyIndicator: FC<
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
		<Popover.Root>
			<Popover.Trigger
				{...restProps}
				onMouseEnter={highlightCommitIds}
				// TODO: we should also clear if the element unmounts
				onMouseLeave={clearHighlightedCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightedCommitIds}
				aria-label={tooltip}
				openOnHover
			/>
			<Popover.Portal>
				<Popover.Positioner
					sideOffset={4}
					// To match tooltips.
					side="top"
				>
					<Popover.Popup render={<TooltipPopup />}>{tooltip}</Popover.Popup>
				</Popover.Positioner>
			</Popover.Portal>
		</Popover.Root>
	);
};
