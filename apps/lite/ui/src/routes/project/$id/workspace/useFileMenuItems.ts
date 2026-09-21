import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import { fileSetMenuItems, reviewedMenuItem } from "./fileSetMenuItems.ts";
import { useFileSetActions, useFileSetSubject } from "./useFileSetActions.ts";
import { fileAddress, type FileAddress } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { TreeChange } from "@gitbutler/but-sdk";

export const useFileMenuItems = ({
	projectId,
	address,
	path,
	change,
	isReviewed,
}: {
	projectId: string;
	address: FileAddress;
	path: string;
	change?: TreeChange;
	/**
	 * Whether the diff on show has been reviewed, which is what the row's own tick
	 * says and which way round the mark reads. Left out where the surface offers
	 * reviewing in its own chrome — the diff header's button — so that the menu
	 * does not say it a second time.
	 */
	isReviewed?: boolean;
}): Array<NativeMenuItem> => {
	// A linked worktree's file is opened and revealed where it lives, not in the
	// project's own checkout; what it can act on is `useFileSetActions`' to say.
	const worktree =
		address.parent._tag === "UncommittedChanges" ? address.parent.worktree : undefined;
	const pathMenuItems = usePathMenuItems({ projectId, path, worktree });
	const actions = useFileSetActions({ projectId, fileParent: address.parent });

	// A file's acts apply to the checked set when the file is part of it, as dragging it does.
	const isChecked = useAppSelector((state) =>
		projectSlice.selectors.selectAddressChecked(state, projectId, fileAddress(address)),
	);
	const subject = useFileSetSubject({
		projectId,
		fileParent: address.parent,
		promote: isChecked,
		own: () => [fileAddress(address)],
		ownCount: 1,
	});

	// A file listed under uncommitted changes without a change is a conflicted one.
	const isWorktreeConflict = !change && address.parent._tag === "UncommittedChanges";

	return nativeMenuItemsFromGroups([
		pathMenuItems,
		...(isWorktreeConflict
			? [
					[
						nativeMenuItem({
							label: "Mark as Resolved",
							enabled: actions.canResolve,
							onSelect: () => actions.markResolved([path]),
						}),
					] satisfies Array<NativeMenuItem>,
				]
			: []),
		...(change ? fileSetMenuItems({ actions, subject, fileParent: address.parent }) : []),
		...(change && isReviewed !== undefined
			? [[reviewedMenuItem({ actions, changes: [change], isReviewed })]]
			: []),
	]);
};
