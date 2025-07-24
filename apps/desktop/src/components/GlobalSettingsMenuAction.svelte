<script lang="ts">
	import { goto } from '$app/navigation';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService.svelte';
	import { inject } from '@gitbutler/shared/context';

	const shortcutService = inject(SHORTCUT_SERVICE);
	shortcutService.on('global-settings', () => {
		goto(newSettingsPath());
	});
</script>
