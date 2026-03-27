import { Match } from "effect";

export type ShortcutActionBase = {
	_tag: string;
};

export type ShortcutBinding<Action extends ShortcutActionBase, Context = void> = {
	id: string;
	description: string;
	keys: Array<string>;
	action: Action;
	repeat?: boolean;
	when?: (context: Context) => boolean;
};

type SharedShortcutAction = { _tag: "TogglePreview" } | { _tag: "ToggleFullscreenPreview" };

export const getShortcutAction = <Action extends ShortcutActionBase, Context>(
	bindings: Array<ShortcutBinding<Action, Context>>,
	context: Context,
	event: KeyboardEvent,
): Action | null => {
	for (const binding of bindings) {
		if (!binding.keys.includes(event.key)) continue;
		if (binding.repeat === false && event.repeat) continue;
		if (binding.when && !binding.when(context)) continue;
		return binding.action;
	}

	return null;
};

export const globalShortcutBindings: Array<ShortcutBinding<SharedShortcutAction>> = [
	{
		id: "toggle-preview",
		description: "preview",
		keys: ["p"],
		action: { _tag: "TogglePreview" },
		repeat: false,
	},
	{
		id: "toggle-fullscreen-preview",
		description: "fullscreen preview",
		keys: ["d"],
		action: { _tag: "ToggleFullscreenPreview" },
		repeat: false,
	},
];

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

export const bindingLabelSuffix = <Action extends ShortcutActionBase, Context>(
	label: string,
	bindings: Array<ShortcutBinding<Action, Context>>,
	actionTag: Action["_tag"],
	options?: {
		extraKeys?: Array<string>;
	},
): string => {
	const binding = bindings.find((binding) => binding.action._tag === actionTag);
	if (!binding) return label;

	return `${label} (${formatShortcutKeys([...binding.keys, ...(options?.extraKeys ?? [])])})`;
};
