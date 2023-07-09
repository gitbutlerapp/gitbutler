import { get, writable, type Writable } from '@square/svelte-store';

const SETTINGS_KEY = 'settings-json';
export const SETTINGS_CONTEXT = Symbol();

export interface Settings {
	aiSummariesEnabled?: boolean;
	trayWidth?: string;
	theme?: string;
	zoom?: number;
}

export type SettingsStore = Writable<Settings>;

export function loadUserSettings(): Writable<Settings> {
	let obj: any;
	try {
		obj = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '');
	} catch {
		obj = {};
	}

	const store = writable(obj);
	return {
		subscribe: store.subscribe,
		set: store.set,
		update: (updater) => {
			store.update(updater);
			localStorage.setItem(SETTINGS_KEY, JSON.stringify(get(store)));
		}
	};
}
