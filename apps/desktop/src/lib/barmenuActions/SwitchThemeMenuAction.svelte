<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { loadUserSettings } from '$lib/settings/userSettings';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { initTheme } from '$lib/utils/theme';
	import { onMount } from 'svelte';

	const userSettings = loadUserSettings();
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
