import { get, writable, type Writable } from 'svelte/store';

const SETTINGS_KEY = 'settings-json';
export const SETTINGS = Symbol('Settings');

export type ScrollbarVisilitySettings = 'scroll' | 'hover' | 'always';

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
	diffFont: string;
	diffLigatures: boolean;
	inlineUnifiedDiffs: boolean;
	defaultCodeEditor: string;
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
	diffFont: 'Geist Mono, Menlo, monospace',
	diffLigatures: false,
	inlineUnifiedDiffs: false,
	defaultCodeEditor: 'vscode'
};

export function loadUserSettings(): Writable<Settings> {
	let obj: any;
	try {
		obj = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '');
	} catch {
		obj = {};
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
