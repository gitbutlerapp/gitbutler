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
						? toElectronAccelerator(outlineHotkeys.insertEmptyCommitAbove.hotkey)
						: undefined,
				onSelect: () => insertBlankCommit("above"),
			}),
			nativeMenuItem({
				label: "Below",
				accelerator: toElectronAccelerator(
					acceleratorSide === "below"
						? outlineHotkeys.insertEmptyCommitAbove.hotkey
						: outlineHotkeys.insertEmptyCommitBelow.hotkey,
				),
				onSelect: () => insertBlankCommit("below"),
			}),
		],
	});
