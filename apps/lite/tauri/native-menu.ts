import { LogicalPosition } from "@tauri-apps/api/dpi";
import {
	Menu,
	type CheckMenuItemOptions,
	type MenuItemOptions,
	type PredefinedMenuItemOptions,
	type SubmenuOptions,
} from "@tauri-apps/api/menu";
import type { NativeMenuPopupItem, ShowNativeMenuParams } from "#electron/ipc.ts";

type MenuEntry =
	| MenuItemOptions
	| CheckMenuItemOptions
	| SubmenuOptions
	| PredefinedMenuItemOptions;

const menuEntries = (
	items: Array<NativeMenuPopupItem>,
	onItem: (itemId: string) => void,
): Array<MenuEntry> =>
	items.map((item): MenuEntry => {
		if (item._tag === "Separator") return { item: "Separator" };
		if (item.submenu)
			return { text: item.label, enabled: item.enabled, items: menuEntries(item.submenu, onItem) };

		return {
			id: item.itemId,
			text: item.label,
			accelerator: item.accelerator,
			checked: item.checked,
			enabled: item.enabled,
			action: item.itemId === undefined ? undefined : onItem,
		};
	});

const showNativeMenu = async ({
	items,
	position,
}: ShowNativeMenuParams): Promise<string | null> => {
	let selectedItemId: string | null = null;
	const menu = await Menu.new({
		items: menuEntries(items, (itemId) => {
			selectedItemId = itemId;
		}),
	});

	// `popup` returns once the menu closes, but the item's action arrives on its own channel; the
	// event loop delivers it ahead of the reply to the `close` that follows.
	try {
		await menu.popup(new LogicalPosition(position.x, position.y));
	} finally {
		await menu.close();
	}

	return selectedItemId;
};

export const handlers = { showNativeMenu };
