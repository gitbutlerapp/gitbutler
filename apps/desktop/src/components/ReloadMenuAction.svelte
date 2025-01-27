<script lang="ts">
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const shortcutService = getContext(ShortcutService);

	shortcutService.on('reload', () => {
		location.reload();
	});
</script>
