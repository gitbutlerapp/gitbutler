import { createKeybindingsHandler } from 'tinykeys';

interface KeybindDefinitions {
	[combo: string]: (event: KeyboardEvent) => void;
}

export const shortcuts = {
	global: {
		open_repository: {
			title: 'Add local repository…',
			description: 'Create a new project by adding a local repository',
			keys: '$mod+O'
		},
		clone_repository: {
			title: 'Clone repository',
			description: 'Clone a remote repository to your local machine',
			keys: '$mod+Shift+O'
		}
	},
	view: {
		zoom_in: {
			title: 'Zoom in',
			description: 'Zoom in UI',
			keys: '$mod++',
			altkeys: '$mod+='
		},
		zoom_out: {
			title: 'Zoom out',
			description: 'Zoom out UI',
			keys: '$mod+-'
		},
		reset_zoom: {
			title: 'Reset zoom',
			description: 'Reset zoom level',
			keys: '$mod+0'
		},
		switch_theme: {
			title: 'Switch theme',
			description: 'Switch between light and dark themes',
			keys: '$mod+T'
		},
		toggle_sidebar: {
			title: 'Toggle sidebar',
			description: 'Show or hide the sidebar',
			keys: '$mod+/'
		},
		reload_view: {
			title: 'Reload view',
			description: 'Reload the current view',
			keys: '$mod+R'
		}
	},
	project: {
		project_history: {
			title: 'Project history',
			description: 'Opens the project history view. Revert changes, view commits, and more.',
			keys: '$mod+Shift+H'
		}
	}
};

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
