import type { FileParent } from "#ui/addresses.ts";
import { fileAddress } from "#ui/addresses.ts";
import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import { fileSetMenuItems, reviewedMenuItem } from "./fileSetMenuItems.ts";
import { useFileSetActions, useFileSetSubject } from "./useFileSetActions.ts";
import type { DirectoryCheckedState } from "./DirectoryRow.tsx";
import type { FileRowItem } from "./file-row.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useMemo } from "react";

/**
 * What a directory offers, which is what the files below it offer, said once. The path
 * acts need nothing but the path — a folder opens in an editor and reveals in the file
 * manager exactly as a file does — so they come straight from {@link usePathMenuItems};
 * the rest are the shared acts, addressed to the subtree.
 *
 * A directory gives way to the checked set on the same terms a file does: a file when it
 * is checked, a folder when every file below it is, which is what its box says.
 */
export const useDirectoryMenuItems = ({
	projectId,
	fileParent,
	path,
	items,
	checkedState,
	isReviewed,
	isCollapsed,
	onToggleCollapsed,
}: {
	projectId: string;
	fileParent: FileParent;
	path: string;
	/** Every file below this directory. */
	items: Array<FileRowItem>;
	checkedState: DirectoryCheckedState;
	/** Whether every change below this directory has been reviewed; the row's tick says the same. */
	isReviewed: boolean;
	isCollapsed: boolean;
	onToggleCollapsed: () => void;
}): Array<NativeMenuItem> => {
	// A directory in a linked worktree opens and reveals where it lives, as a file row's
	// does; what it can act on is `useFileSetActions`' to say, for both alike.
	const worktree = fileParent._tag === "UncommittedChanges" ? fileParent.worktree : undefined;
	const pathMenuItems = usePathMenuItems({ projectId, path, worktree });
	const actions = useFileSetActions({ projectId, fileParent });

	// A conflict has nothing to commit or discard yet, so it is no part of what the acts
	// below are addressed to — only of what "Mark as Resolved" is.
	//
	// Split by hand, in one pass: a directory stands for every file below it, so this walks
	// the whole subtree, and the compiler gives a scope only to the half that feeds a hook.
	const { changes, conflictPaths } = useMemo(() => {
		const changes: Array<TreeChange> = [];
		const conflictPaths: Array<string> = [];
		for (const item of items) {
			if (item._tag === "Change") changes.push(item.change);
			else conflictPaths.push(item.path);
		}
		return { changes, conflictPaths };
	}, [items]);

	const subject = useFileSetSubject({
		projectId,
		fileParent,
		promote: checkedState === "checked",
		own: () => changes.map(({ path }) => fileAddress({ parent: fileParent, path })),
		ownCount: changes.length,
	});

	return nativeMenuItemsFromGroups([
		pathMenuItems,
		...(conflictPaths.length > 0
			? [
					[
						nativeMenuItem({
							label:
								conflictPaths.length > 1
									? `Mark ${conflictPaths.length.toLocaleString()} Files as Resolved`
									: "Mark as Resolved",
							enabled: actions.canResolve,
							onSelect: () => actions.markResolved(conflictPaths),
						}),
					] satisfies Array<NativeMenuItem>,
				]
			: []),
		...(changes.length > 0
			? [
					...fileSetMenuItems({ actions, subject, fileParent }),
					[reviewedMenuItem({ actions, changes, isReviewed })],
				]
			: []),
		[
			nativeMenuItem({
				label: isCollapsed ? "Expand" : "Collapse",
				accelerator: toElectronAccelerator(changesFileHotkeys.toggleFoldDirectory.hotkey),
				onSelect: onToggleCollapsed,
			}),
		],
	]);
};
