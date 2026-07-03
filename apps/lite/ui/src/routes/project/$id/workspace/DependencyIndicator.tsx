import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import { Array, pipe } from "effect";
import { ComponentProps, FC } from "react";

export const DependencyIndicator: FC<
	{
		projectId: string;
		commitIds: Array.NonEmptyArray<string>;
		branchNameByCommitId: (commitId: string) => string | undefined;
	} & ComponentProps<"button">
> = ({ projectId, commitIds, branchNameByCommitId, ...restProps }) => {
	const dispatch = useAppDispatch();
	const branchNames = pipe(
		commitIds,
		Array.flatMapNullable((commitId) => branchNameByCommitId(commitId)),
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
