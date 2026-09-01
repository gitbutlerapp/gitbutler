import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";

/**
 * What a directory offers, which is what the files below it offer, said once. The path
 * acts need nothing but the path — a folder opens in an editor and reveals in the file
 * manager exactly as a file does — so they come straight from {@link usePathMenuItems}.
 */
export const useDirectoryMenuItems = ({
	projectId,
	path,
	items,
	isCollapsed,
	onToggleCollapsed,
}: {
	projectId: string;
	path: string;
	/** Every file below this directory. */
	items: Array<FileRowItem>;
	isCollapsed: boolean;
	onToggleCollapsed: () => void;
}): Array<NativeMenuItem> => {
	const pathMenuItems = usePathMenuItems({ projectId, path });

	return nativeMenuItemsFromGroups([
		pathMenuItems,
		[
			nativeMenuItem({
				label: isCollapsed ? "Expand" : "Collapse",
				accelerator: toElectronAccelerator(changesFileHotkeys.toggleFoldDirectory.hotkey),
				onSelect: onToggleCollapsed,
			}),
		],
	]);
};
