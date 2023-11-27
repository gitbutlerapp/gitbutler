import { get, writable, type Writable } from 'svelte/store';

const SETTINGS_KEY = 'settings-json';
export const SETTINGS_CONTEXT = Symbol();

export interface Settings {
	aiSummariesEnabled?: boolean;
	bottomPanelExpanded: boolean;
	bottomPanelHeight: number;
	peekTrayWidth: number;
	theme?: string;
	trayWidth: number;
	vbranchExpandableHeight: number;
	stashedBranchesHeight: number;
	defaultLaneWidth: number;
	defaultFileWidth: number;
	defaultTreeHeight: number;
	zoom: number;
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
	vbranchExpandableHeight: 150,
	stashedBranchesHeight: 150,
	zoom: 1
};

export type SettingsStore = Writable<Settings>;

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
