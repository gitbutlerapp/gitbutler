<script lang="ts">
	import { BACKEND } from '$lib/backend';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { initTheme } from '$lib/utils/theme';
	import { inject } from '@gitbutler/shared/context';

	const userSettings = inject(SETTINGS);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const backend = inject(BACKEND);

	initTheme(userSettings, backend);

	function updateTheme() {
		userSettings.update((s) => ({
			...s,
			theme: s.theme === 'light' ? 'dark' : 'light'
		}));
	}

	$effect(() => shortcutService.on('switch-theme', () => updateTheme()));
</script>
