import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { Tooltip } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import type { FileRowTooltipPayload } from "./FileRowTooltip.tsx";

export const DependencyIndicator: FC<
	{
		projectId: string;
		commitIds: Array<string>;
		branchNameByCommitId: (commitId: string) => string | undefined;
		tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	} & ComponentProps<"button">
> = ({ projectId, commitIds, branchNameByCommitId, tooltipHandle, ...restProps }) => {
	const dispatch = useAppDispatch();
	const branchNames = new Set(
		commitIds.flatMap((commitId) => branchNameByCommitId(commitId) ?? []),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";
	const highlightCommitIds = () => {
		dispatch(
			projectSlice.actions.setHighlightedCommitIds({
				projectId,
				commitIds,
			}),
		);
	};
	const clearHighlightedCommitIds = () => {
		dispatch(projectSlice.actions.setHighlightedCommitIds({ projectId, commitIds: null }));
	};

	return (
		<Tooltip.Trigger
			{...restProps}
			handle={tooltipHandle}
			payload={{ content: tooltip }}
			onMouseEnter={highlightCommitIds}
			// TODO: we should also clear if the element unmounts
			onMouseLeave={clearHighlightedCommitIds}
			onFocus={highlightCommitIds}
			onBlur={clearHighlightedCommitIds}
			aria-label={tooltip}
		/>
	);
};
