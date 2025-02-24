import { browser } from '$app/environment';
import { persisted } from '@gitbutler/shared/persisted';
import { derived, writable, type Readable } from 'svelte/store';

export type Theme = 'light' | 'dark' | 'system';

// Create a persisted store for the theme preference
export const themeStore = persisted<Theme>('system', 'web-theme-preference');

// Create a writable store for system theme that can be updated
const systemThemeStore = writable<'light' | 'dark'>('light');

// Create a derived store for the effective theme
export const effectiveThemeStore: Readable<'light' | 'dark'> = derived(
	[themeStore, systemThemeStore],
	([$themeStore, $systemThemeStore]) => {
		return $themeStore === 'system' ? $systemThemeStore : ($themeStore as 'light' | 'dark');
	}
);

/**
 * Update system theme store based on media query result
 */
function updateSystemTheme(matches: boolean) {
	systemThemeStore.set(matches ? 'dark' : 'light');
}

/**
 * Initialize theme system - sets up system theme detection and applies theme to DOM
 */
export function initTheme() {
	if (!browser) return;

	// Set up system theme detection
	const darkModeQuery = window.matchMedia('(prefers-color-scheme: dark)');

	// Set initial system theme
	updateSystemTheme(darkModeQuery.matches);

	// Listen for system theme changes
	darkModeQuery.addEventListener('change', (e) => {
		updateSystemTheme(e.matches);
	});

	// Subscribe to effective theme changes and update DOM
	const unsubscribe = effectiveThemeStore.subscribe((effectiveTheme) => {
		updateDom(effectiveTheme);
	});

	// Return cleanup function (optional, for potential future use)
	return unsubscribe;
}

/**
 * Update the DOM with the current theme
 */
function updateDom(effectiveTheme: 'light' | 'dark') {
	if (!browser) return;

	const docEl = document.documentElement;

	if (effectiveTheme === 'dark') {
		docEl.classList.remove('light');
		docEl.classList.add('dark');
		docEl.style.colorScheme = 'dark';
	} else {
		docEl.classList.remove('dark');
		docEl.classList.add('light');
		docEl.style.colorScheme = 'light';
	}
}

/**
 * Set the theme preference
 */
export function setTheme(theme: Theme) {
	themeStore.set(theme);
}
