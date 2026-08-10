import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { type NativeMenuItem, nativeMenuItem } from "#ui/native-menu.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";

/**
 * Switches every file list between flat and folder-shaped. Both modes are named
 * rather than folded into one checkbox, so the menu says what it is offering
 * either way round, and the tick reports which is in force.
 */
export const useFileDisplayModeMenuItems = (): Array<NativeMenuItem> => {
	const mode = useFileDisplayMode();
	const { mutate: saveGUISettings } = useSaveGUISettings();

	return [
		nativeMenuItem({
			label: "View as List",
			checked: mode === "list",
			onSelect: () => saveGUISettings({ fileDisplayMode: "list" }),
		}),
		nativeMenuItem({
			label: "View as Tree",
			checked: mode === "tree",
			onSelect: () => saveGUISettings({ fileDisplayMode: "tree" }),
		}),
	];
};
