import { Match } from "effect";

export type ShortcutActionBase = {
	_tag: string;
};

export type ShortcutBinding<Action extends ShortcutActionBase> = {
	id: string;
	description: string;
	keys: Array<string>;
	action: Action;
	repeat?: boolean;
};

const normalizeShortcutKey = (key: string): string => (key.length === 1 ? key.toLowerCase() : key);

type ParsedShortcutKey = {
	key: string;
	altKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
};

const parseShortcutKey = (shortcutKey: string): ParsedShortcutKey => {
	const parsed: ParsedShortcutKey = {
		key: "",
		altKey: false,
		ctrlKey: false,
		metaKey: false,
		shiftKey: false,
	};

	for (const part of shortcutKey.split("+"))
		switch (part) {
			case "Alt":
				parsed.altKey = true;
				break;
			case "Ctrl":
				parsed.ctrlKey = true;
				break;
			case "Meta":
				parsed.metaKey = true;
				break;
			case "Shift":
				parsed.shiftKey = true;
				break;
			default:
				parsed.key = normalizeShortcutKey(part);
		}

	return parsed;
};

const eventMatchesShortcutKey = (event: KeyboardEvent, shortcutKey: string): boolean => {
	const parsed = parseShortcutKey(shortcutKey);
	if (parsed.key === "") return false;

	return (
		normalizeShortcutKey(event.key) === parsed.key &&
		event.altKey === parsed.altKey &&
		event.ctrlKey === parsed.ctrlKey &&
		event.metaKey === parsed.metaKey &&
		event.shiftKey === parsed.shiftKey
	);
};

export const getAction = <Action extends ShortcutActionBase>(
	bindings: Array<ShortcutBinding<Action>>,
	event: KeyboardEvent,
): Action | null => {
	for (const binding of bindings) {
		if (!binding.keys.some((shortcutKey) => eventMatchesShortcutKey(event, shortcutKey))) continue;
		if (binding.repeat === false && event.repeat) continue;
		return binding.action;
	}

	return null;
};

const formatShortcutKey = (key: string): string => {
	const parsed = parseShortcutKey(key);
	if (parsed.key === "") return key;

	const modifiers = [
		parsed.ctrlKey ? "ctrl" : null,
		parsed.altKey ? "alt" : null,
		parsed.shiftKey ? "shift" : null,
		parsed.metaKey ? "meta" : null,
	].filter((modifier) => modifier !== null);

	const formattedKey = Match.value(parsed.key).pipe(
		Match.when("ArrowUp", () => "↑"),
		Match.when("ArrowDown", () => "↓"),
		Match.when("ArrowLeft", () => "←"),
		Match.when("ArrowRight", () => "→"),
		Match.when("Escape", () => "esc"),
		Match.when("Enter", () => "enter"),
		Match.orElse(() => parsed.key),
	);

	return [...modifiers, formattedKey].join("+");
};

export const formatShortcutKeys = (keys: Array<string>): string =>
	keys.map((key) => formatShortcutKey(key)).join("/");

export const bindingButtonLabel = (binding: ShortcutBinding<ShortcutActionBase>): string =>
	`${binding.description} (${formatShortcutKeys(binding.keys)})`;
