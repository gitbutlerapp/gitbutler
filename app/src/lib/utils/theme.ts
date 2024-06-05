import { appWindow, type Theme } from '@tauri-apps/api/window';
import { writable, type Writable } from 'svelte/store';
import type { Settings } from '$lib/settings/userSettings';

export const theme = writable('dark');

let systemTheme: string | null;
let selectedTheme: string | undefined;

export function initTheme(userSettings: Writable<Settings>) {
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
		selectedTheme === 'dark' ||
		(selectedTheme === 'system' && systemTheme === 'dark') ||
		(selectedTheme === undefined && systemTheme === 'dark')
	) {
		docEl.classList.remove('light');
		docEl.classList.add('dark');
		docEl.style.colorScheme = 'dark';
	} else if (
		selectedTheme === 'light' ||
		(selectedTheme === 'system' && systemTheme === 'light') ||
		(selectedTheme === undefined && systemTheme === 'light')
	) {
		docEl.classList.remove('dark');
		docEl.classList.add('light');
		docEl.style.colorScheme = 'light';
	}
}
