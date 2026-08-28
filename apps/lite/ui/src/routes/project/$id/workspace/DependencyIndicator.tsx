import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppStore } from "#ui/store.ts";
import { Tooltip } from "@base-ui/react";
import { type ComponentProps, type FC, useEffect, useEffectEvent, useRef } from "react";
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
	const store = useAppStore();
	const ownedCommitIds = useRef<Set<string> | null>(null);

	const branchNames = new Set(
		commitIds
			.values()
			.map((commitId) => branchNameByCommitId(commitId))
			.filter((x) => x != null),
	);
	const tooltip =
		branchNames.size > 0
			? `Depends on ${branchNames.values().toArray().join(", ")}`
			: "Unknown dependencies";

	const highlightCommitIds = () => {
		dispatch(
			projectSlice.actions.setDependencyCommitIds({
				projectId,
				commitIds,
			}),
		);

		ownedCommitIds.current = projectSlice.selectors.selectDependencyCommitIds(
			store.getState(),
			projectId,
		);
	};

	const clearHighlightedCommitIds = () => {
		ownedCommitIds.current = null;

		dispatch(projectSlice.actions.setDependencyCommitIds({ projectId, commitIds: null }));
	};

	// Virtualisation can unmount a hovered or focused indicator without firing leave or blur.
	// Read the latest state without making the mount-scoped effect reactive, and clear only the
	// Set this indicator installed so its cleanup cannot erase a newer indicator's highlight.
	const clearHighlightedCommitIdsOnUnmount = useEffectEvent(() => {
		if (
			projectSlice.selectors.selectDependencyCommitIds(store.getState(), projectId) ===
			ownedCommitIds.current
		)
			clearHighlightedCommitIds();
	});

	useEffect(() => () => clearHighlightedCommitIdsOnUnmount(), []);

	return (
		<Tooltip.Trigger
			{...restProps}
			handle={tooltipHandle}
			payload={{ content: tooltip }}
			onMouseEnter={highlightCommitIds}
			onMouseLeave={clearHighlightedCommitIds}
			onFocus={highlightCommitIds}
			onBlur={clearHighlightedCommitIds}
			aria-label={tooltip}
		/>
	);
};
