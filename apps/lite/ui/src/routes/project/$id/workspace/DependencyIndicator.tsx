import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { HighlightedCommitIdsContext } from "#ui/HighlightedCommitIdsContext.ts";
import { Tooltip } from "@base-ui/react";
import { ComponentProps, FC, use } from "react";

export const DependencyIndicator: FC<
	{
		commitIds: Array<string>;
		branchNameByCommitId: (commitId: string) => string | undefined;
	} & ComponentProps<"button">
> = ({ commitIds, branchNameByCommitId, ...restProps }) => {
	const { setHighlightedCommitIds, clearHighlightedCommitIds } = use(HighlightedCommitIdsContext);
	const branchNames = new Set(
		commitIds.flatMap((commitId) => branchNameByCommitId(commitId) ?? []),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";
	const highlightCommitIds = () => setHighlightedCommitIds(commitIds);

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
