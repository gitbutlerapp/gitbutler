import { appWindow, type Theme } from '@tauri-apps/api/window';
import { writable } from 'svelte/store';
import type { SettingsStore } from '$lib/settings/userSettings';

export const theme = writable('dark');

let systemTheme: string | null;
let selectedTheme: string | undefined;

export function initTheme(userSettings: SettingsStore) {
	appWindow.theme().then((value: Theme | null) => {
		systemTheme = value;
		updateDom();
	});
	appWindow.onThemeChanged((e) => {
		systemTheme = e.payload;
		updateDom();
	});
	userSettings.subscribe((s) => {
		selectedTheme = s.theme;
		updateDom();
	});
}

export function updateDom() {
	const docEl = document.documentElement;
	if (
		selectedTheme == 'dark' ||
		(selectedTheme == 'system' && systemTheme == 'dark') ||
		(selectedTheme == undefined && systemTheme == 'dark')
	) {
		docEl.classList.add('dark');
		docEl.style.colorScheme = 'dark';
	} else if (
		selectedTheme == 'light' ||
		(selectedTheme == 'system' && systemTheme == 'light') ||
		(selectedTheme == undefined && systemTheme == 'light')
	) {
		docEl.classList.remove('dark');
		docEl.style.colorScheme = 'light';
	}
}
