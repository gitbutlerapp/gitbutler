import type { NativeMenuPopupItem, NativeMenuPosition } from "#electron/ipc.ts";
import type { MouseEvent as ReactMouseEvent } from "react";

type NativeMenuAction = () => void | Promise<void>;

export type NativeMenuItemData = {
	label: string;
	accelerator?: string;
	/** Renders the item as a checkbox in the given state. */
	checked?: boolean;
	enabled?: boolean;
	onSelect?: NativeMenuAction;
	submenu?: Array<NativeMenuItem>;
};

export type NativeMenuItem = { _tag: "Separator" } | ({ _tag: "Item" } & NativeMenuItemData);

/** @public */
export const nativeMenuSeparator: NativeMenuItem = {
	_tag: "Separator",
};

/** @public */
export const nativeMenuItem = (item: NativeMenuItemData): NativeMenuItem => ({
	_tag: "Item",
	...item,
});

/** @public */
export const nativeMenuItemsFromGroups = (
	groups: Array<Array<NativeMenuItem>>,
): Array<NativeMenuItem> =>
	groups.flatMap((group, idx) =>
		idx !== groups.length - 1 ? [...group, nativeMenuSeparator] : group,
	);

const serializeNativeMenuItems = (
	items: Array<NativeMenuItem>,
	handlers: Map<string, NativeMenuAction | undefined>,
	nextActionId: { value: number },
): Array<NativeMenuPopupItem> =>
	items.map((item): NativeMenuPopupItem => {
		if (item._tag === "Separator") return { _tag: "Separator" };

		if (item.submenu) {
			return {
				_tag: "Item",
				label: item.label,
				accelerator: item.accelerator,
				enabled: item.enabled,
				submenu: serializeNativeMenuItems(item.submenu, handlers, nextActionId),
			};
		}

		const itemId = `native-menu:${nextActionId.value++}`;
		handlers.set(itemId, item.onSelect);

		return {
			_tag: "Item",
			label: item.label,
			accelerator: item.accelerator,
			checked: item.checked,
			enabled: item.enabled,
			itemId,
		};
	});

/** Optional context the caller wants the host to know about (e.g. the file path). */
type NativeMenuContext = {
	path?: string;
};

const showNativeMenu = async (
	items: Array<NativeMenuItem>,
	position: NativeMenuPosition,
	context?: NativeMenuContext,
): Promise<void> => {
	if (items.length === 0) return;

	const handlers = new Map<string, NativeMenuAction | undefined>();
	const serializedItems = serializeNativeMenuItems(items, handlers, { value: 0 });

	const selectedItemId = await window.lite.showNativeMenu({
		items: serializedItems,
		position,
		context,
	});
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
	event: ReactMouseEvent<HTMLElement> | MouseEvent,
	items: Array<NativeMenuItem>,
	context?: NativeMenuContext,
): Promise<void> => {
	event.preventDefault();

	const position =
		event.clientX === 0 && event.clientY === 0 && event.currentTarget instanceof HTMLElement
			? getBottomLeft(event.currentTarget)
			: {
					x: Math.round(event.clientX),
					// Position just below the cursor so the first item is not hovered on
					// open.
					y: Math.round(event.clientY) + 1,
				};

	await showNativeMenu(items, position, context);
};

export const showNativeMenuFromTrigger = async (
	trigger: HTMLElement,
	items: Array<NativeMenuItem>,
	context?: NativeMenuContext,
): Promise<void> => showNativeMenu(items, getBottomLeft(trigger), context);
