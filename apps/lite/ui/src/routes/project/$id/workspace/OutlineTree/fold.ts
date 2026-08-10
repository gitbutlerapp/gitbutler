import { decodeBytes } from "#ui/api/bytes.ts";
import { branchOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { AppDispatch } from "#ui/store.ts";

/**
 * Fold or unfold a branch segment. Every fold surface (chevron click, context
 * menu, z hotkey) goes through here so the policy cannot diverge.
 *
 * With `select`, the branch row also takes the outline selection. Callers pass
 * it when the fold hides the selected commit (it would otherwise fall out of
 * the navigation index, making selection jump to the top of the outline);
 * unfolds and folds of unrelated segments leave the selection alone.
 */
export const toggleFoldedSegment = (
	dispatch: AppDispatch,
	{
		projectId,
		branchRefBytes,
		select,
	}: { projectId: string; branchRefBytes: Array<number>; select: boolean },
) => {
	if (select) {
		dispatch(
			projectSlice.actions.selectOutline({
				projectId,
				selection: branchOperand({ branchRef: branchRefBytes }),
			}),
		);
	}
	dispatch(
		projectSlice.actions.toggleSegmentFolded({
			projectId,
			branchRef: decodeBytes(branchRefBytes),
		}),
	);
};
