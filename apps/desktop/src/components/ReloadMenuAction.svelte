<script lang="ts">
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService.svelte';
	import { inject } from '@gitbutler/shared/context';

	const shortcutService = inject(SHORTCUT_SERVICE);

	shortcutService.on('reload', () => {
		location.reload();
	});
</script>
