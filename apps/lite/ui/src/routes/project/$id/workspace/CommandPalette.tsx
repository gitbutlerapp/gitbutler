import { Kbd } from "#ui/components/Kbd.tsx";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { type CommandGroup } from "#ui/hotkeys.ts";
import {
	getHotkeyManager,
	getSequenceManager,
	Hotkey,
	HotkeySequence,
	useHotkeyRegistrations,
} from "@tanstack/react-hotkeys";
import { Order } from "effect";
import { type FC } from "react";

type CommandPaletteItem = {
	group: CommandGroup;
	id: string;
	name: string;
	hotkey: Hotkey | HotkeySequence;
	type: "hotkey" | "sequence";
};

type Props = {
	open: boolean;
	onOpenChange: (open: boolean) => void;
};

const groupCommandPaletteItems = (
	items: Array<CommandPaletteItem>,
): Array<PickerDialogGroup<CommandPaletteItem>> => {
	const grouped = Map.groupBy(items, (item) => item.group);

	return Array.from(grouped.entries())
		.toSorted(Order.mapInput(Order.string, ([group]) => group))
		.map(
			([group, items]): PickerDialogGroup<CommandPaletteItem> => ({
				value: group,
				items: items.toSorted(Order.mapInput(Order.string, (item) => item.name)),
			}),
		);
};

export const CommandPalette: FC<Props> = ({ open, onOpenChange }) => {
	const { hotkeys, sequences } = useHotkeyRegistrations();
	const hotkeyItems: Array<CommandPaletteItem> = [
		...hotkeys.flatMap((hotkey): CommandPaletteItem | [] =>
			hotkey.options.enabled !== false && hotkey.options.meta?.name !== undefined
				? {
						group: hotkey.options.meta.group,
						id: hotkey.id,
						name: hotkey.options.meta.name,
						hotkey: hotkey.hotkey,
						type: "hotkey",
					}
				: [],
		),
		...sequences.flatMap((sequence): CommandPaletteItem | [] =>
			sequence.options.enabled !== false && sequence.options.meta?.name !== undefined
				? {
						group: sequence.options.meta.group,
						id: sequence.id,
						name: sequence.options.meta.name,
						hotkey: sequence.sequence,
						type: "sequence",
					}
				: [],
		),
	];
	const items = groupCommandPaletteItems(hotkeyItems);

	const runHotkey = (item: CommandPaletteItem) => {
		onOpenChange(false);
		if (item.type === "hotkey") getHotkeyManager().triggerRegistration(item.id);
		else getSequenceManager().triggerSequence(item.id);
	};

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No hotkeys found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.name}
			getItemType={(x) => <Kbd hotkey={x.hotkey} />}
			items={items}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={runHotkey}
			placeholder="Search hotkeys…"
		/>
	);
};
