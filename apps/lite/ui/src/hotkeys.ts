import {
	formatForDisplay,
	normalizeRegisterableHotkey,
	type Hotkey,
	type HotkeyMeta,
	type RegisterableHotkey,
} from "@tanstack/react-hotkeys";

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

export type CommandGroup =
	| "Branch"
	| "Commit"
	| "Diff"
	| "Display"
	| "File"
	| "Global"
	| "Operations log"
	| "Stack"
	| "Uncommitted changes"
	| "Workspace";

declare module "@tanstack/react-hotkeys" {
	interface HotkeyMeta {
		group: CommandGroup;
	}
}

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

export const toElectronAccelerator = (hotkey: RegisterableHotkey): string | undefined => {
	const accelerator = normalizeRegisterableHotkey(hotkey)
		.split("+")
		.map((part) => electronAcceleratorKeys[part as HotkeySegment<Hotkey>] ?? part)
		.join("+");

	return accelerator.length > 0 ? accelerator : undefined;
};

type HotkeyWithMeta = {
	hotkey: RegisterableHotkey;
	meta?: HotkeyMeta;
};

export const globalHotkeys = {
	commandPalette: {
		hotkey: "Mod+K",
	},
	redo: {
		hotkey: "Mod+Shift+Z",
		meta: { group: "Operations log", name: "Redo" },
	},
	selectProject: {
		hotkey: "Mod+Shift+P",
		meta: { group: "Global", name: "Select project" },
	},
	undo: {
		hotkey: "Mod+Z",
		meta: { group: "Operations log", name: "Undo" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const workspaceHotkeys = {
	applyBranch: {
		hotkey: "Mod+Shift+A",
		meta: { group: "Workspace", name: "Apply branch" },
	},
	createIndependentBranch: {
		hotkey: "Mod+N",
		meta: { group: "Workspace", name: "Add new branch" },
	},
	updateWorkspace: {
		hotkey: "Alt+Shift+R",
		meta: {
			group: "Workspace",
			name: "Update workspace (rebases all stacks)",
		},
	},
	focusPreviousSelectionScope: {
		hotkey: "Mod+Alt+ArrowLeft",
	},
	focusNextSelectionScope: {
		hotkey: "Mod+Alt+ArrowRight",
	},
	toggleFiles: {
		hotkey: "F",
		meta: { group: "Diff", name: "Toggle files" },
	},
	toggleOutline: {
		hotkey: ".",
		meta: { group: "Display", name: "Toggle outline" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const outlineHotkeys = {
	absorb: {
		hotkey: "A",
		meta: { group: "Uncommitted changes", name: "Absorb" },
	},
	amendCommit: {
		hotkey: "Shift+A",
		meta: { group: "Commit", name: "Amend commit" },
	},
	composeCommitHere: {
		hotkey: "C",
	},
	checkCommit: {
		hotkey: "Space",
		meta: { group: "Commit", name: "Check commit" },
	},
	checkBranchCommits: {
		hotkey: "Space",
		meta: { group: "Branch", name: "Check branch commits" },
	},
	insertEmptyCommit: {
		hotkey: "N",
		meta: {
			group: "Commit",
			name: "Insert empty commit",
		},
	},
	createDependentBranchAbove: {
		hotkey: "B",
		meta: { group: "Branch", name: "Create dependent branch above" },
	},
	openCommitInBrowser: {
		hotkey: "O",
		meta: { group: "Commit", name: "Open commit in browser" },
	},
	openPRInBrowser: {
		hotkey: "O",
		meta: { group: "Branch", name: "Open pull request in browser" },
	},
	setCommitTarget: {
		hotkey: "Shift+C",
		meta: { group: "Commit", name: "Set commit target" },
	},
	composeCommitMessage: {
		hotkey: "Shift+Z",
	},
	deleteCommit: {
		hotkey: globalThis.window.lite.platform === "darwin" ? "Mod+Backspace" : "Delete",
		meta: { group: "Commit", name: "Delete commit" },
	},
	composeCommitMessageFromChanges: {
		hotkey: "R",
	},
	moveCommitDown: {
		hotkey: "Alt+ArrowDown",
		meta: { group: "Commit", name: "Move commit down" },
	},
	moveCommitUp: {
		hotkey: "Alt+ArrowUp",
		meta: { group: "Commit", name: "Move commit up" },
	},
	workspaceBranchAndAncestorsPush: {
		hotkey: "Shift+P",
		meta: { group: "Branch", name: "Push with branches below" },
	},
	updateStack: {
		hotkey: "Alt+R",
		meta: { group: "Stack", name: "Update stack (rebases)" },
	},
	renameBranch: {
		hotkey: "R",
		meta: { group: "Branch", name: "Rename branch" },
	},
	rewordCommit: {
		hotkey: "R",
		meta: { group: "Commit", name: "Reword commit" },
	},
	selectBranch: {
		hotkey: "T",
		meta: { group: "Workspace", name: "Jump to branch" },
	},
	selectChanges: {
		hotkey: "Z",
	},
} satisfies Record<string, HotkeyWithMeta>;

export const changesHotkeys = {
	amendCommit: {
		hotkey: "Mod+Alt+Enter",
		meta: { group: "Uncommitted changes", name: "Amend" },
	},
	commit: {
		hotkey: "Mod+Enter",
		meta: { group: "Uncommitted changes", name: "Commit" },
	},
	selectCommitTarget: {
		hotkey: "Mod+Shift+B",
	},
} satisfies Record<string, HotkeyWithMeta>;

export const changesFileHotkeys = {
	absorb: {
		hotkey: "A",
		meta: { group: "File", name: "Absorb" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const pullRequestHotkeys = {
	update: {
		hotkey: "Mod+Enter",
	},
} satisfies Record<string, HotkeyWithMeta>;

export const selectionOperationHotkeys = {
	move: {
		hotkey: "M",
	},
	cut: {
		hotkey: "Mod+X",
	},
} satisfies Record<string, HotkeyWithMeta>;

export const operationHotkeys = {
	cancel: {
		hotkey: "Escape",
	},
	confirm: {
		hotkey: "Enter",
	},
	confirmTransfer: {
		hotkey: "Mod+V",
	},
	selectAbove: {
		hotkey: "A",
	},
	selectBelow: {
		hotkey: "B",
	},
	selectInto: {
		hotkey: "I",
	},
} satisfies Record<string, HotkeyWithMeta>;

export const diffHotkeys = {
	toggleDiffStyle: {
		hotkey: "Mod+B",
		meta: { group: "Diff", name: "Toggle diff style" },
	},
} satisfies Record<string, HotkeyWithMeta>;
