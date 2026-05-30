import {
	normalizeRegisterableHotkey,
	type Hotkey,
	type HotkeyMeta,
	type RegisterableHotkey,
} from "@tanstack/react-hotkeys";

export type CommandGroup =
	| "Branch"
	| "Branches"
	| "Changes file"
	| "Changes"
	| "Commit file"
	| "Commit"
	| "Details"
	| "Files"
	| "Global"
	| "Outline"
	| "Operation mode"
	| "Panels"
	| "Rename branch"
	| "Reword commit"
	| "Stack";

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
	hotkey: Hotkey;
	meta: HotkeyMeta;
};

export const globalHotkeys = {
	commandPalette: {
		hotkey: "Mod+K",
		meta: { group: "Global", name: "Command palette" },
	},
	redo: {
		hotkey: "Mod+Shift+Z",
		meta: { group: "Outline", name: "Redo" },
	},
	selectProject: {
		hotkey: "Mod+Shift+P",
		meta: { group: "Global", name: "Select project" },
	},
	undo: {
		hotkey: "Mod+Z",
		meta: { group: "Outline", name: "Undo" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const workspaceHotkeys = {
	applyBranch: {
		hotkey: "Mod+Shift+A",
		meta: { group: "Branches", name: "Apply branch" },
	},
	focusPreviousPanel: {
		hotkey: "H",
		meta: { group: "Panels", name: "Focus previous panel" },
	},
	focusNextPanel: {
		hotkey: "L",
		meta: { group: "Panels", name: "Focus next panel" },
	},
	toggleFilesPanel: {
		hotkey: "F",
		meta: { group: "Files", name: "Toggle files" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const outlineHotkeys = {
	absorb: {
		hotkey: "A",
		meta: { group: "Changes", name: "Absorb" },
	},
	amendCommit: {
		hotkey: "Shift+A",
		meta: { group: "Commit", name: "Amend commit" },
	},
	composeCommitHere: {
		hotkey: "C",
		meta: { group: "Commit", name: "Compose commit here" },
	},
	composeCommitMessage: {
		hotkey: "Shift+Z",
		meta: { group: "Outline", name: "Compose commit message" },
	},
	editChangesCommitMessage: {
		hotkey: "Enter",
		meta: { group: "Changes", name: "Compose commit message" },
	},
	moveCommitDown: {
		hotkey: "Alt+ArrowDown",
		meta: { group: "Commit", name: "Move commit down" },
	},
	moveCommitUp: {
		hotkey: "Alt+ArrowUp",
		meta: { group: "Commit", name: "Move commit up" },
	},
	renameBranch: {
		hotkey: "Enter",
		meta: { group: "Branch", name: "Rename branch" },
	},
	rewordCommit: {
		hotkey: "Enter",
		meta: { group: "Commit", name: "Reword commit" },
	},
	selectBranch: {
		hotkey: "T",
		meta: { group: "Outline", name: "Select branch" },
	},
	selectChanges: {
		hotkey: "Z",
		meta: { group: "Outline", name: "Select changes" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const changesHotkeys = {
	amendCommit: {
		hotkey: "Mod+Alt+Enter",
		meta: { group: "Changes", name: "Amend" },
	},
	commit: {
		hotkey: "Mod+Enter",
		meta: { group: "Changes", name: "Commit" },
	},
	selectCommitBranch: {
		hotkey: "Mod+Shift+B",
		meta: { group: "Changes", name: "Select commit branch" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const changesFileHotkeys = {
	absorb: {
		hotkey: "A",
		meta: { group: "Changes file", name: "Absorb" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const operationHotkeys = {
	cancel: {
		hotkey: "Escape",
		meta: { group: "Operation mode", name: "Cancel" },
	},
	confirm: {
		hotkey: "Enter",
		meta: { group: "Operation mode", name: "Confirm" },
	},
	confirmTransfer: {
		hotkey: "Mod+V",
		meta: { group: "Operation mode", name: "Confirm" },
	},
	selectMoveAbove: {
		hotkey: "A",
		meta: { group: "Operation mode", name: "Select move above" },
	},
	selectMoveBelow: {
		hotkey: "B",
		meta: { group: "Operation mode", name: "Select move below" },
	},
	selectRub: {
		hotkey: "R",
		meta: { group: "Operation mode", name: "Select rub" },
	},
} satisfies Record<string, HotkeyWithMeta>;
