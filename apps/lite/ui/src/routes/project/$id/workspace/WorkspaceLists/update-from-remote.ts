import { decodeBytes } from "#ui/api/bytes.ts";
import { branchAddress } from "#ui/addresses.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { setCursor } from "#ui/use-cursor.ts";
import type { AppDispatch } from "#ui/store.ts";

/**
 * Every entry point opens the dialog through here, so the select-branch-first
 * step cannot diverge between them.
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
