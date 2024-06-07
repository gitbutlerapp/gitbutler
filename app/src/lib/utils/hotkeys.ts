import { createKeybindingsHandler } from 'tinykeys';

interface KeybindDefinitions {
	[combo: string]: (event: KeyboardEvent) => void;
}

export function createKeybind(keybinds: KeybindDefinitions[]) {
	const keys: KeybindDefinitions = {};

	Object.entries(keybinds).forEach(([combo, callback]) => {
		keys[combo] = (event: KeyboardEvent) => {
			if (
				event.repeat ||
				event.target instanceof HTMLInputElement ||
				event.target instanceof HTMLTextAreaElement
			)
				return;

			event.preventDefault();

			callback(event);
		};
	});

	return createKeybindingsHandler(keys);
}
