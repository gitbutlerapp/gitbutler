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
	| "File"
	| "Global"
	| "Operations log"
	| "Stack"
	| "Uncommitted changes"
	| "Outline"
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
	fetchFromRemotes: {
		hotkey: "Alt+Shift+F",
		meta: { group: "Workspace", name: "Fetch" },
	},
	updateWorkspace: {
		hotkey: "Alt+Shift+R",
		meta: {
			group: "Workspace",
			name: "Update workspace (rebases all stacks)",
		},
	},
	focusHorizontalSelectionScopeLeft: {
		hotkey: "Mod+Alt+ArrowLeft",
	},
	focusHorizontalSelectionScopeRight: {
		hotkey: "Mod+Alt+ArrowRight",
	},
	focusVerticalSelectionScopeUp: {
		hotkey: "Mod+Alt+ArrowUp",
	},
	focusVerticalSelectionScopeDown: {
		hotkey: "Mod+Alt+ArrowDown",
	},
	settings: {
		hotkey: "Mod+,",
		meta: { group: "Workspace", name: "Settings" },
	},
	toggleFiles: {
		hotkey: "F",
		meta: { group: "Diff", name: "Toggle files" },
	},
	toggleOutline: {
		hotkey: ".",
		meta: { group: "Global", name: "Toggle outline" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const branchesHotkeys = {
	deleteBranchRef: {
		hotkey: globalThis.window.lite.platform === "darwin" ? "Mod+Backspace" : "Delete",
		meta: { group: "Branch", name: "Delete branch reference" },
	},
} satisfies Record<string, HotkeyWithMeta>;

export const outlineHotkeys = {
	checkCommit: {
		hotkey: "Space",
		meta: { group: "Commit", name: "Check commit" },
	},
	checkBranchCommits: {
		hotkey: "Space",
		meta: { group: "Branch", name: "Check branch commits" },
	},
	insertEmptyCommitAbove: {
		hotkey: "N",
		meta: {
			group: "Commit",
			name: "Insert empty commit above",
		},
	},
	insertEmptyCommitBelow: {
		hotkey: "Shift+N",
		meta: {
			group: "Commit",
			name: "Insert empty commit below",
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
	composeCommitMessage: {
		hotkey: "Shift+Z",
	},
	deleteBranchRef: {
		hotkey: "Mod+Alt+Backspace",
		meta: { group: "Branch", name: "Delete branch reference" },
	},
	deleteCommit: {
		hotkey: globalThis.window.lite.platform === "darwin" ? "Mod+Backspace" : "Delete",
		meta: { group: "Commit", name: "Delete commit" },
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
	uncommitCommit: {
		hotkey: "Mod+Alt+Backspace",
		meta: { group: "Commit", name: "Uncommit" },
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
	checkFile: {
		hotkey: "Space",
		meta: { group: "File", name: "Check file" },
	},
	discard: {
		hotkey: "Mod+Backspace",
		meta: { group: "File", name: "Discard changes" },
	},
	filter: {
		hotkey: "Mod+F",
		meta: { group: "File", name: "Filter files" },
	},
	openInEditor: {
		hotkey: "E",
		meta: { group: "File", name: "Open in editor" },
	},
	uncommit: {
		hotkey: "Mod+Alt+Backspace",
		meta: { group: "File", name: "Uncommit" },
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
	foldFile: {
		hotkey: "Mod+Alt+[",
		meta: { group: "Diff", name: "Fold" },
	},
	unfoldFile: {
		hotkey: "Mod+Alt+]",
		meta: { group: "Diff", name: "Unfold" },
	},
	toggleDiffStyle: {
		hotkey: "Mod+B",
		meta: { group: "Diff", name: "Toggle diff style" },
	},
	openInEditor: {
		hotkey: "E",
		meta: { group: "Diff", name: "Open in editor" },
	},
} satisfies Record<string, HotkeyWithMeta>;
