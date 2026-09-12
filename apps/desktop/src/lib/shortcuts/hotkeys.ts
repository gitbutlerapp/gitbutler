import { createKeybindingsHandler } from "tinykeys";

interface KeybindDefinitions {
	[combo: string]: (event: KeyboardEvent) => void;
}

/**
 * Whether `event` is `platform`'s "select all" chord.
 *
 * macOS selects all with ⌘ and leaves Ctrl+A as "move to the start of the line", the emacs
 * binding its text fields carry everywhere else, so accepting either modifier there takes an
 * editing key away rather than adding a shortcut. Everywhere else Ctrl is the one that selects.
 *
 * `platform` is a `Backend["platformName"]`, which the desktop app gets from the OS rather
 * than from the user agent.
 */
export function isSelectAllChord(event: KeyboardEvent, platform: string): boolean {
	if (event.key !== "a") return false;

	return platform === "macos" ? event.metaKey : event.ctrlKey;
}

export const shortcuts = {
	global: {
		open_repository: {
			title: "Add local repository…",
			keys: "$mod+O",
			description: null,
		},
		clone_repository: {
			title: "Clone repository",
			keys: "$mod+Shift+O",
			description: null,
		},
		create_branch: {
			title: "Create branch",
			keys: "$mod+B",
			description: null,
		},
		create_dependent_branch: {
			title: "Create dependent branch",
			keys: "$mod+Shift+B",
			description: null,
		},
	},
	view: {
		switch_theme: {
			title: "Switch theme",
			keys: "$mod+T",
			description: null,
		},
		toggle_sidebar: {
			title: "Toggle Unassigned",
			keys: "$mod+\\",
			description: null,
		},
		zoom_in: {
			title: "Zoom in",
			keys: "$mod+=",
			description: null,
		},
		zoom_out: {
			title: "Zoom out",
			keys: "$mod+-",
			description: null,
		},
		reset_zoom: {
			title: "Reset zoom",
			keys: "$mod+0",
			description: null,
		},
		reload_view: {
			title: "Reload view",
			keys: "$mod+R",
			description: null,
		},
	},
	project: {
		project_history: {
			title: "Project history",
			description: "Opens the project history view. Revert changes, view commits, and more.",
			keys: "$mod+Shift+H",
		},
	},
};

export function createKeybind(keybinds: KeybindDefinitions) {
	const keys: KeybindDefinitions = {
		// Ignore backspace keydown events always
		Backspace: () => {},
	};

	for (const [combo, callback] of Object.entries(keybinds)) {
		keys[combo] = (event: KeyboardEvent) => {
			if (
				event.repeat ||
				event.target instanceof HTMLInputElement ||
				event.target instanceof HTMLTextAreaElement ||
				("_lexicalHandled" in event && event._lexicalHandled)
			) {
				return;
			}

			event.preventDefault();

			callback(event);
		};
	}

	return createKeybindingsHandler(keys);
}
