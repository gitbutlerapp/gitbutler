<script lang="ts">
	import { goto } from '$app/navigation';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const shortcutService = getContext(ShortcutService);
	shortcutService.on('global-settings', () => {
		goto(newSettingsPath());
	});
</script>
