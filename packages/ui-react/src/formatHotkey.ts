import { formatForDisplay } from "@tanstack/react-hotkeys";

const modifierOrder = ["⌃", "⌥", "⇧", "⌘"];

const sortModifiers = (keys: Array<string>): Array<string> => [
	...keys
		.filter((key) => modifierOrder.includes(key))
		.toSorted((a, b) => modifierOrder.indexOf(a) - modifierOrder.indexOf(b)),
	...keys.filter((key) => !modifierOrder.includes(key)),
];

// This wrapper ensures the format matches Apple's HIG and thereby also what
// we show in context menus.
// https://github.com/TanStack/hotkeys/issues/136
export const formatForDisplaySorted = (hotkey: Parameters<typeof formatForDisplay>[0]): string =>
	sortModifiers(formatForDisplay(hotkey).split(" ")).join(" ");
