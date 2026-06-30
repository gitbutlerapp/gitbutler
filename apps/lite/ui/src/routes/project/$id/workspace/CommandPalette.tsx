import { Kbd } from "#ui/components/Kbd.tsx";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { type CommandGroup } from "#ui/hotkeys.ts";
import {
	getHotkeyManager,
	getSequenceManager,
	Hotkey,
	HotkeyMeta,
	HotkeyOptions,
	HotkeySequence,
	SequenceOptions,
	useHotkeyRegistrations,
} from "@tanstack/react-hotkeys";
import { Order } from "effect";
import { useState, type FC } from "react";

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

const isEnabled = <T extends HotkeyOptions | SequenceOptions>(
	opts: T,
	activeElement: Element | null,
): opts is T & { meta: HotkeyMeta & { name: string } } =>
	opts.enabled !== false &&
	opts.meta?.name !== undefined &&
	(!opts.target ||
		opts.target === document ||
		opts.target === window ||
		opts.target === activeElement);

export const CommandPalette: FC<Props> = ({ open, onOpenChange }) => {
	const [initialActiveElement] = useState(() => document.activeElement);

	const { hotkeys, sequences } = useHotkeyRegistrations();
	const hotkeyItems: Array<CommandPaletteItem> = [
		...hotkeys.flatMap((hotkey): CommandPaletteItem | [] =>
			isEnabled(hotkey.options, initialActiveElement)
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
			isEnabled(sequence.options, initialActiveElement)
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
