<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { initTheme } from '$lib/utils/theme';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const shortcutService = getContext(ShortcutService);

	initTheme(userSettings);

	function updateTheme() {
		userSettings.update((s) => ({
			...s,
			theme: s.theme === 'light' ? 'dark' : 'light'
		}));
	}

	shortcutService.on('switch-theme', () => {
		updateTheme();
	});
</script>
