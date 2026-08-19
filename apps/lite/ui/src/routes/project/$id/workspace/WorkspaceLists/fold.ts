import { decodeBytes } from "#ui/api/bytes.ts";
import { setCursor } from "#ui/use-cursor.ts";
import { branchAddress } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { AppDispatch } from "#ui/store.ts";

/**
 * Fold or unfold a branch segment. Every fold surface (chevron click, context
 * menu, z hotkey) goes through here so the policy cannot diverge.
 *
 * With `select`, the branch row also takes the sidebar selection. Callers pass
 * it when the fold hides the selected commit (it would otherwise fall out of
 * the address space, making selection jump to the top of the sidebar);
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
	if (select) setCursor("applied", branchAddress({ branchRef: branchRefBytes }));

	dispatch(
		projectSlice.actions.toggleSegmentFolded({
			projectId,
			branchRef: decodeBytes(branchRefBytes),
		}),
	);
};
