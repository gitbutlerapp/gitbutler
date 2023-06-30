import { building } from '$app/environment';
import { writable, type Writable } from '@square/svelte-store';
import { appWindow, type Theme } from '@tauri-apps/api/window';

const themeStorageKey = 'theme';
export const theme: Writable<string> = writable('dark');

let systemTheme: string | null;

export function initTheme() {
	if (building) return;
	appWindow.theme().then((value: Theme | null) => {
		systemTheme = value;
		updateDom();
	});
	appWindow.onThemeChanged((e) => {
		systemTheme = e.payload;
		updateDom();
	});
}

export function setTheme(name: string) {
	localStorage.setItem(themeStorageKey, name);
	theme.set(name);
	updateDom();
}

export function getTheme(): string | null {
	return localStorage.getItem(themeStorageKey);
}

export function updateDom() {
	const docEl = document.documentElement;
	const selectedTheme = localStorage.getItem(themeStorageKey);
	if (selectedTheme == 'dark' || (selectedTheme == 'system' && systemTheme == 'dark')) {
		docEl.classList.add('dark');
		docEl.style.colorScheme = 'dark';
		theme.set('dark');
	} else if (selectedTheme == 'light' || (selectedTheme == 'system' && systemTheme == 'light')) {
		docEl.classList.remove('dark');
		docEl.style.colorScheme = 'light';
		theme.set('light');
	}
}
