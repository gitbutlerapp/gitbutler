<script lang="ts">
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/shared/context';

	const shortcutService = inject(SHORTCUT_SERVICE);

	$effect(() => shortcutService.on('reload', () => location.reload()));
</script>
