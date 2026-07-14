import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { HighlightedCommitIdsContext } from "#ui/HighlightedCommitIdsContext.ts";
import { Tooltip } from "@base-ui/react";
import { ComponentProps, FC, use } from "react";

export const DependencyIndicator: FC<
	{
		projectId: string;
		commitIds: Array<string>;
		branchNameByCommitId: (commitId: string) => string | undefined;
	} & ComponentProps<"button">
> = ({ projectId, commitIds, branchNameByCommitId, ...restProps }) => {
	const { setHighlightedCommitIds, clearHighlightedCommitIds } = use(HighlightedCommitIdsContext);
	const branchNames = new Set(
		commitIds.flatMap((commitId) => branchNameByCommitId(commitId) ?? []),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";
	const highlightCommitIds = () => setHighlightedCommitIds(projectId, commitIds);
	const clearHighlightCommitIds = () => clearHighlightedCommitIds(projectId);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				{...restProps}
				onMouseEnter={highlightCommitIds}
				// TODO: we should also clear if the element unmounts
				onMouseLeave={clearHighlightCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightCommitIds}
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
