import { InjectionToken } from '@gitbutler/core/context';
import { type ScrollbarVisilitySettings } from '@gitbutler/ui';
import { platform } from '@tauri-apps/plugin-os';
import { get, writable, type Writable } from 'svelte/store';

const SETTINGS_KEY = 'settings-json';
export const SETTINGS = new InjectionToken<Writable<Settings>>('Settings');

export type CodeEditorSettings = {
	schemeIdentifer: string;
	displayName: string;
};

export type TerminalSettings = {
	identifier: string;
	displayName: string;
	platform: 'macos' | 'windows' | 'linux';
};

function defaultTerminalForPlatform(): TerminalSettings {
	switch (platform()) {
		case 'windows':
			return { identifier: 'powershell', displayName: 'PowerShell', platform: 'windows' };
		case 'linux':
			return { identifier: 'gnome-terminal', displayName: 'GNOME Terminal', platform: 'linux' };
		default:
			return { identifier: 'terminal', displayName: 'Terminal', platform: 'macos' };
	}
}

export interface Settings {
	aiSummariesEnabled?: boolean;
	bottomPanelExpanded: boolean;
	bottomPanelHeight: number;
	peekTrayWidth: number;
	theme?: string;
	trayWidth: number;
	stashedBranchesHeight: number;
	defaultLaneWidth: number;
	defaultFileWidth: number;
	defaultTreeHeight: number;
	zoom: number;
	scrollbarVisibilityState: ScrollbarVisilitySettings;
	tabSize: number;
	wrapText: boolean;
	diffFont: string;
	diffLigatures: boolean;
	inlineUnifiedDiffs: boolean;
	strongContrast: boolean;
	colorBlindFriendly: boolean;
	defaultCodeEditor: CodeEditorSettings;
	defaultTerminal: TerminalSettings;
	defaultFileListMode: 'tree' | 'list';
	pathFirst: boolean;
	singleDiffView: boolean;
}

const defaults: Settings = {
	aiSummariesEnabled: false,
	bottomPanelExpanded: false,
	bottomPanelHeight: 200,
	peekTrayWidth: 480,
	trayWidth: 320,
	defaultLaneWidth: 460,
	defaultFileWidth: 460,
	defaultTreeHeight: 100,
	stashedBranchesHeight: 150,
	zoom: 1,
	scrollbarVisibilityState: 'scroll',
	tabSize: 4,
	wrapText: false,
	diffFont: 'Geist Mono, Menlo, monospace',
	diffLigatures: false,
	inlineUnifiedDiffs: false,
	strongContrast: false,
	colorBlindFriendly: false,
	defaultCodeEditor: { schemeIdentifer: 'vscode', displayName: 'VSCode' },
	defaultTerminal: { identifier: 'terminal', displayName: 'Terminal', platform: 'macos' },
	defaultFileListMode: 'list',
	pathFirst: true,
	singleDiffView: false
};

export function loadUserSettings(): Writable<Settings> {
	let obj: any;
	try {
		obj = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '');
	} catch {
		obj = {};
	}

	// If no terminal was persisted, resolve to the platform default.
	if (!obj.defaultTerminal) {
		obj.defaultTerminal = defaultTerminalForPlatform();
	}

	const store = writable<Settings>({ ...defaults, ...obj });
	return {
		subscribe: store.subscribe,
		set: store.set,
		update: (updater) => {
			store.update(updater);
			localStorage.setItem(SETTINGS_KEY, JSON.stringify(get(store)));
		}
	};
}
