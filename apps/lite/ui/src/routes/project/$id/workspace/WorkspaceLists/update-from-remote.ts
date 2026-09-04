import { decodeBytes } from "#ui/api/bytes.ts";
import { branchAddress } from "#ui/addresses.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { setCursor } from "#ui/use-cursor.ts";
import type { AppDispatch } from "#ui/store.ts";

/**
 * Open the update-from-remote dialog for a branch. Every entry point — the
 * remote leg's button, the branch menu — goes through here, so the
 * select-branch-first step, which leaves the details on the branch once the
 * dialog closes, cannot diverge between them.
 */
export const openUpdateFromRemote = (
	dispatch: AppDispatch,
	branchRefBytes: Array<number>,
): void => {
	setCursor("applied", branchAddress({ branchRef: branchRefBytes }));
	dispatch(
		interfaceSlice.actions.openDialog({
			dialog: { _tag: "UpdateFromRemote", branchRef: decodeBytes(branchRefBytes) },
		}),
	);
};
