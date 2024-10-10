<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { initTheme } from '$lib/utils/theme';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	initTheme(userSettings);

	function updateTheme() {
		userSettings.update((s) => ({
			...s,
			theme: s.theme === 'light' ? 'dark' : 'light'
		}));
	}

	onMount(() => {
		const unsubscribeTheme = listen<string>('menu://view/switch-theme/clicked', updateTheme);

		return async () => {
			unsubscribeTheme();
		};
	});

	const handleKeyDown = createKeybind({
		'$mod+T': updateTheme
	});
</script>

<svelte:window on:keydown={handleKeyDown} />
