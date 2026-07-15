import { useProjectStore } from "#ui/store.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import { ComponentProps, FC } from "react";

export const DependencyIndicator: FC<
	{
		projectId: string;
		commitIds: Array<string>;
		branchNameByCommitId: (commitId: string) => string | undefined;
	} & ComponentProps<"button">
> = ({ projectId, commitIds, branchNameByCommitId, ...restProps }) => {
	const projectStore = useProjectStore(projectId);
	const branchNames = new Set(
		commitIds.flatMap((commitId) => branchNameByCommitId(commitId) ?? []),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";
	const highlightCommitIds = () => {
		projectStore.setHighlightedCommitIds(commitIds);
	};
	const clearHighlightedCommitIds = () => {
		projectStore.setHighlightedCommitIds(null);
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
