import { Match } from "effect";

// We'll need something more sophisticated as we scale, but this is a start.
export type ShortcutBinding<Action, Context = void> = {
	id: string;
	description: string;
	keys: Array<string>;
	action: Action;
	repeat?: boolean;
	when?: (context: Context) => boolean;
};

export const shortcutKeys = {
	togglePreview: "p",
	toggleFullscreenPreview: "d",
} as const;

export type SharedShortcutAction = { _tag: "TogglePreview" } | { _tag: "ToggleFullscreenPreview" };

export const getShortcutAction = <Action, Context>(
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
		keys: [shortcutKeys.togglePreview],
		action: { _tag: "TogglePreview" },
		repeat: false,
	},
	{
		id: "toggle-fullscreen-preview",
		description: "fullscreen preview",
		keys: [shortcutKeys.toggleFullscreenPreview],
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
				Match.orElse(() => key.toLowerCase()),
			),
		)
		.join("/");
