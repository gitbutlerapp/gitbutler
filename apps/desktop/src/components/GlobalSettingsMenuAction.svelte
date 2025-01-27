<script lang="ts">
	import { settingsPath } from '$lib/routes/routes.svelte';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const shortcutService = getContext(ShortcutService);
	shortcutService.on('settings', () => {
		goto(settingsPath());
	});
</script>
