import {
	type HotkeySequence,
	type UseHotkeySequenceOptions,
	type UseHotkeyOptions,
	type RegisterableHotkey,
	normalizeRegisterableHotkey,
	useHotkeys,
	UseHotkeyDefinition,
	useHotkeySequences,
	UseHotkeySequenceDefinition,
	Hotkey,
} from "@tanstack/react-hotkeys";
import { Context, createContext, useContext, useEffect, useId } from "react";
import type { CommandGroup } from "./groups";
import type { NativeMenuItem, NativeMenuItemData } from "#ui/native-menu.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { commandsActions, CommandRegistrationId } from "./state";

// consider if many of these could typically share a label
export type CommandOptions = {
	group: CommandGroup;
	/** @default true */
	enabled?: boolean;
	commandPalette?: {
		label: string;
	};
	shortcutsBar?: {
		label: string;
	};
	contextMenu?: NativeMenuItemData;
	hotkeys?: Array<CommandHotkey | CommandHotkeySequence>;
};

type CommandHotkey = {
	hotkey: RegisterableHotkey;
} & Omit<
	UseHotkeyOptions,
	// Causes a type error with Immer.
	"target"
>;

type CommandHotkeySequence = {
	sequence: HotkeySequence;
} & Omit<
	UseHotkeySequenceOptions,
	// Causes a type error with Immer.
	"target"
>;

type CommandTrigger = "commandPalette" | "contextMenu" | "hotkey" | "ui";

export type CommandFn = (scenario: CommandTrigger) => void;

export const CommandFnContext: Context<Map<CommandRegistrationId, CommandFn> | undefined> =
	createContext<Map<CommandRegistrationId, CommandFn> | undefined>(undefined);

type HotkeySegment<T extends string> = T extends `${infer Head}+${infer Tail}`
	? Head | HotkeySegment<Tail>
	: T;

const electronAcceleratorKeys: Partial<Record<HotkeySegment<Hotkey>, string>> = {
	Alt: "Alt",
	ArrowDown: "Down",
	ArrowLeft: "Left",
	ArrowRight: "Right",
	ArrowUp: "Up",
	Backspace: "Backspace",
	Control: "Control",
	Delete: "Delete",
	End: "End",
	Escape: "Esc",
	Enter: "Enter",
	Home: "Home",
	Meta: "Command",
	Mod: "CommandOrControl",
	PageDown: "PageDown",
	PageUp: "PageUp",
	Shift: "Shift",
	Space: "Space",
	Tab: "Tab",
};

const toElectronAccelerator = (hotkey: RegisterableHotkey): string | undefined => {
	const accelerator = normalizeRegisterableHotkey(hotkey)
		.split("+")
		.map((part) => electronAcceleratorKeys[part as HotkeySegment<Hotkey>] ?? part)
		.join("+");

	return accelerator.length > 0 ? accelerator : undefined;
};

const firstNativeMenuAccelerator = (
	hotkeys: Array<CommandHotkey | CommandHotkeySequence> | undefined,
): string | undefined => {
	const firstHotkey = hotkeys?.find(
		(hotkey): hotkey is CommandHotkey => !("sequence" in hotkey) && hotkey.enabled !== false,
	);

	return firstHotkey ? toElectronAccelerator(firstHotkey.hotkey) : undefined;
};

type ResolvedCommand<F extends CommandFn, O extends CommandOptions> = {
	commandFn: F;
	contextMenu: O extends { contextMenu: NativeMenuItemData } ? NativeMenuItem : undefined;
	hotkeys: O extends { hotkeys: Array<CommandHotkey | CommandHotkeySequence> }
		? Array<Hotkey | HotkeySequence>
		: undefined;
};

// future: maybe add useCommands. and/or internal multi hotkeys like useHotkeys
// separating the function from options improves ability to memo by ref of options obj
/**
 * Hotkeys are automatically disabled when a layer of higher precedence is enabled with the same
 * keybind.
 */
export const useCommand = <F extends CommandFn, O extends CommandOptions>(
	commandFn: F,
	options: O,
): ResolvedCommand<F, O> => {
	const id = useId();
	// oxlint-disable-next-line typescript/no-non-null-assertion: Let it loudly fail.
	const cbmap = useContext(CommandFnContext)!;
	const dispatch = useAppDispatch();
	const regs = useAppSelector((state) => state.commands.registrations);
	const regOptions = regs[id];

	useEffect(() => {
		dispatch(commandsActions.register({ id, options }));

		return () => void dispatch(commandsActions.deregister({ id }));
	}, [dispatch, id, options]);

	useEffect(() => {
		cbmap.set(id, commandFn);

		return () => void cbmap.delete(id);
	}, [cbmap, id, commandFn]);

	const { hotkeyDefs, sequenceDefs, resolvedHotkeys } = (regOptions?.hotkeys ?? []).reduce(
		(acc, hk) => {
			const defEnabled = regOptions && regOptions.enabled !== false && hk.enabled !== false;

			if (defEnabled)
				acc.resolvedHotkeys.push(
					"sequence" in hk ? hk.sequence : normalizeRegisterableHotkey(hk.hotkey),
				);

			const def: UseHotkeyDefinition | UseHotkeySequenceDefinition = {
				// We only want to be warned if two conflicting hotkeys are enabled at the same time. NB we
				// must therefore be wary of which keys we use directly with useHotkey(s).
				callback: () => commandFn("hotkey"),
				options: {
					enabled: defEnabled,
					conflictBehavior: defEnabled ? "warn" : "allow",
					// Allow overriding any of our default behavior if you really want to...
					...hk,
				},
				// ...at both layers, since the shapes don't align. Irrelevant keys are ignored.
				...hk,
			};

			if ("sequence" in hk) acc.sequenceDefs.push(def as UseHotkeySequenceDefinition);
			else acc.hotkeyDefs.push(def as UseHotkeyDefinition);

			return acc;
		},
		{
			hotkeyDefs: [] as Array<UseHotkeyDefinition>,
			sequenceDefs: [] as Array<UseHotkeySequenceDefinition>,
			resolvedHotkeys: [] as Array<Hotkey | HotkeySequence>,
		},
	);

	useHotkeys(hotkeyDefs);
	useHotkeySequences(sequenceDefs);

	return {
		commandFn,
		hotkeys: resolvedHotkeys.length > 0 ? resolvedHotkeys : undefined,
		contextMenu: regOptions?.contextMenu
			? ({
					enabled: regOptions.enabled !== false,
					onSelect: () => commandFn("contextMenu"),
					accelerator: firstNativeMenuAccelerator(regOptions.hotkeys),
					...regOptions.contextMenu,
					_tag: "Item",
				} satisfies NativeMenuItem)
			: undefined,
	} as ResolvedCommand<F, O>;
};

export const useCommandFn = (): ((id: CommandRegistrationId) => CommandFn | undefined) => {
	// oxlint-disable-next-line typescript/no-non-null-assertion: Let it loudly fail.
	const map = useContext(CommandFnContext)!;
	return (id) => map.get(id);
};
