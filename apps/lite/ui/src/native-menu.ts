import type { NativeMenuPopupItem, NativeMenuPosition } from "#electron/ipc.ts";
import { MouseEvent } from "react";

type NativeMenuAction = () => void | Promise<void>;

type NativeMenuItemData = {
	label: string;
	enabled?: boolean;
	onSelect?: NativeMenuAction;
	submenu?: Array<NativeMenuItem>;
};

export type NativeMenuItem = { _tag: "Separator" } | ({ _tag: "Item" } & NativeMenuItemData);

const serializeNativeMenuItems = (
	items: Array<NativeMenuItem>,
	handlers: Map<string, NativeMenuAction | undefined>,
	nextActionId: { value: number },
): Array<NativeMenuPopupItem> =>
	items.map((item): NativeMenuPopupItem => {
		if (item._tag === "Separator") return { _tag: "Separator" };

		if (item.submenu)
			return {
				_tag: "Item",
				label: item.label,
				enabled: item.enabled,
				submenu: serializeNativeMenuItems(item.submenu, handlers, nextActionId),
			};

		const itemId = `native-menu:${nextActionId.value++}`;
		handlers.set(itemId, item.onSelect);

		return {
			_tag: "Item",
			label: item.label,
			enabled: item.enabled,
			itemId,
		};
	});

const showNativeMenu = async (
	items: Array<NativeMenuItem>,
	position: NativeMenuPosition,
): Promise<void> => {
	if (items.length === 0) return;

	const handlers = new Map<string, NativeMenuAction | undefined>();
	const serializedItems = serializeNativeMenuItems(items, handlers, { value: 0 });

	const selectedItemId = await window.lite.showNativeMenu({ items: serializedItems, position });
	if (selectedItemId === null) return;
	await handlers.get(selectedItemId)?.();
};

const getBottomLeft = (element: HTMLElement): NativeMenuPosition => {
	const rect = element.getBoundingClientRect();
	return {
		x: Math.round(rect.left),
		y: Math.round(rect.bottom),
	};
};

export const showNativeContextMenu = async (
	event: MouseEvent<HTMLElement>,
	items: Array<NativeMenuItem>,
): Promise<void> => {
	event.preventDefault();

	const position =
		event.clientX === 0 && event.clientY === 0
			? getBottomLeft(event.currentTarget)
			: {
					x: Math.round(event.clientX),
					// Position just below the cursor so the first item is not hovered on
					// open.
					y: Math.round(event.clientY) + 1,
				};

	await showNativeMenu(items, position);
};

export const showNativeMenuFromTrigger = async (
	trigger: HTMLElement,
	items: Array<NativeMenuItem>,
): Promise<void> => showNativeMenu(items, getBottomLeft(trigger));
