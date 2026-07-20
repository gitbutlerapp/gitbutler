import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import { ComponentProps, FC } from "react";

export const DependencyIndicator: FC<
	{
		projectId: string;
		changeIds: Array<string>;
		branchNameByChangeId: (commitId: string) => string | undefined;
	} & ComponentProps<"button">
> = ({ projectId, changeIds, branchNameByChangeId, ...restProps }) => {
	const dispatch = useAppDispatch();
	const branchNames = new Set(
		changeIds.flatMap((changeId) => branchNameByChangeId(changeId) ?? []),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";
	const highlightCommitIds = () => {
		dispatch(
			projectSlice.actions.setHighlightedChangeIds({
				projectId,
				changeIds,
			}),
		);
	};
	const clearHighlightedCommitIds = () => {
		dispatch(projectSlice.actions.setHighlightedChangeIds({ projectId, changeIds: null }));
	};

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				{...restProps}
				onMouseEnter={highlightCommitIds}
				// TODO: we should also clear if the element unmounts
				onMouseLeave={clearHighlightedCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightedCommitIds}
				aria-label={tooltip}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
