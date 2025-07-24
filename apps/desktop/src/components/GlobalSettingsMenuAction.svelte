<script lang="ts">
	import { goto } from '$app/navigation';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/shared/context';

	const shortcutService = inject(SHORTCUT_SERVICE);
	$effect(() =>
		shortcutService.on('global-settings', () => {
			goto(newSettingsPath());
		})
	);
</script>
