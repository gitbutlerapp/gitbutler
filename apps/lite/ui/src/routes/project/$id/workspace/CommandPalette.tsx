import { Kbd } from "#ui/components/Kbd.tsx";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import type { CommandGroup } from "#ui/hotkeys.ts";
import { iteratorConcat } from "#ui/iterator.ts";
import {
	getHotkeyManager,
	getSequenceManager,
	type Hotkey,
	type HotkeyMeta,
	type HotkeyOptions,
	type HotkeySequence,
	type SequenceOptions,
	useHotkeyRegistrations,
} from "@tanstack/react-hotkeys";
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
	items: Iterable<CommandPaletteItem>,
): Array<PickerDialogGroup<CommandPaletteItem>> => {
	const grouped = Map.groupBy(items, (item) => item.group);

	return Array.from(grouped.entries())
		.toSorted(([a], [b]) => a.localeCompare(b))
		.map(
			([group, items]): PickerDialogGroup<CommandPaletteItem> => ({
				value: group,
				items: items.toSorted((a, b) => a.name.localeCompare(b.name)),
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

	const hotkeyItems: IteratorObject<CommandPaletteItem> = iteratorConcat(
		hotkeys
			.values()
			.map((hotkey): CommandPaletteItem | null =>
				isEnabled(hotkey.options, initialActiveElement)
					? {
							group: hotkey.options.meta.group,
							id: hotkey.id,
							name: hotkey.options.meta.name,
							hotkey: hotkey.hotkey,
							type: "hotkey",
						}
					: null,
			)
			.filter((x) => x != null),
		sequences
			.values()
			.map((sequence): CommandPaletteItem | null =>
				isEnabled(sequence.options, initialActiveElement)
					? {
							group: sequence.options.meta.group,
							id: sequence.id,
							name: sequence.options.meta.name,
							hotkey: sequence.sequence,
							type: "sequence",
						}
					: null,
			)
			.filter((x) => x != null),
	);
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
			itemToStringValue={(x) => `${x.group}: ${x.name}`}
			items={items}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={runHotkey}
			placeholder="Search hotkeys…"
		/>
	);
};
