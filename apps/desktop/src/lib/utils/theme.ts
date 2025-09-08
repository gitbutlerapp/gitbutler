import { type Writable } from 'svelte/store';
import type { IBackend } from '$lib/backend';
import type { Settings } from '$lib/settings/userSettings';

let systemTheme: string | null;
let selectedTheme: string | undefined;

export function initTheme(userSettings: Writable<Settings>, backend: IBackend) {
	backend.systemTheme.subscribe((theme) => {
		systemTheme = theme;
		updateDom();
	});
	userSettings.subscribe((s) => {
		selectedTheme = s.theme;
		updateDom();
	});
}

function updateDom() {
	const docEl = document.documentElement;
	if (
		selectedTheme === 'dark' ||
		(selectedTheme === 'system' && systemTheme === 'dark') ||
		(selectedTheme === undefined && systemTheme === 'dark')
	) {
		docEl.classList.remove('light', 'color-blind');
		docEl.classList.add('dark');
		docEl.style.colorScheme = 'dark';
	} else if (selectedTheme === 'color-blind') {
		docEl.classList.remove('dark', 'light');
		docEl.classList.add('color-blind');
		docEl.style.colorScheme = 'light';
	} else if (
		selectedTheme === 'light' ||
		(selectedTheme === 'system' && systemTheme === 'light') ||
		(selectedTheme === undefined && systemTheme === 'light')
	) {
		docEl.classList.remove('dark', 'color-blind');
		docEl.classList.add('light');
		docEl.style.colorScheme = 'light';
	}
}
