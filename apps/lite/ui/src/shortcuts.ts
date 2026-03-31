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

export const getAction = <Action extends ShortcutActionBase>(
	bindings: Array<ShortcutBinding<Action>>,
	event: KeyboardEvent,
): Action | null => {
	for (const binding of bindings) {
		if (!binding.keys.includes(event.key)) continue;
		if (binding.repeat === false && event.repeat) continue;
		return binding.action;
	}

	return null;
};

export const formatShortcutKeys = (keys: Array<string>): string =>
	keys
		.map((key) =>
			Match.value(key).pipe(
				Match.when("ArrowUp", () => "↑"),
				Match.when("ArrowDown", () => "↓"),
				Match.when("ArrowLeft", () => "←"),
				Match.when("ArrowRight", () => "→"),
				Match.when("Escape", () => "esc"),
				Match.when("Enter", () => "enter"),
				Match.orElse(() => key),
			),
		)
		.join("/");

export const bindingButtonLabel = (binding: ShortcutBinding<ShortcutActionBase>): string =>
	`${binding.description} (${formatShortcutKeys(binding.keys)})`;
