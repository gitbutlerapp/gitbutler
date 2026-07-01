import { nativeMenuItem } from "#ui/native-menu.ts";
import { outlineHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";

export const insertBlankCommitMenuItem = (
	insertBlankCommit: (side: "above" | "below") => void,
	acceleratorSide: "above" | "below",
) =>
	nativeMenuItem({
		label: "Add Empty Commit",
		submenu: [
			nativeMenuItem({
				label: "Above",
				accelerator:
					acceleratorSide === "above"
						? toElectronAccelerator(outlineHotkeys.insertEmptyCommit.hotkey)
						: undefined,
				onSelect: () => insertBlankCommit("above"),
			}),
			nativeMenuItem({
				label: "Below",
				accelerator:
					acceleratorSide === "below"
						? toElectronAccelerator(outlineHotkeys.insertEmptyCommit.hotkey)
						: undefined,
				onSelect: () => insertBlankCommit("below"),
			}),
		],
	});
