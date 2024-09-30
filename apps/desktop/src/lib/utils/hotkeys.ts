import { createKeybindingsHandler } from 'tinykeys';

interface KeybindDefinitions {
	[combo: string]: (event: KeyboardEvent) => void;
}

export enum KeyName {
	Space = ' ',
	Meta = 'Meta',
	Alt = 'Alt',
	Ctrl = 'Ctrl',
	Enter = 'Enter',
	Escape = 'Escape',
	Tab = 'Tab',
	Up = 'ArrowUp',
	Down = 'ArrowDown',
	Left = 'ArrowLeft',
	Right = 'ArrowRight',
	Delete = 'Backspace'
}

export function createKeybind(keybinds: KeybindDefinitions) {
	const keys: KeybindDefinitions = {
		// Ignore backspace keydown events always
		Backspace: () => {}
	};

	for (const [combo, callback] of Object.entries(keybinds)) {
		keys[combo] = (event: KeyboardEvent) => {
			if (
				event.repeat ||
				event.target instanceof HTMLInputElement ||
				event.target instanceof HTMLTextAreaElement
			) {
				return;
			}

			event.preventDefault();

			callback(event);
		};
	}

	return createKeybindingsHandler(keys);
}

export function onMetaEnter(callback: () => void) {
	return (e: KeyboardEvent) => {
		if (e.key === KeyName.Enter && (e.metaKey || e.ctrlKey)) {
			callback();
		}
	};
}
