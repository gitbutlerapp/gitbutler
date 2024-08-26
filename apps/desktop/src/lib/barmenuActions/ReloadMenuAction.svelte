<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { onMount } from 'svelte';

	onMount(() => {
		const unsubscribeReload = listen<string>('menu://view/reload/clicked', () => {
			location.reload();
		});

		return async () => {
			unsubscribeReload();
		};
	});

	const handleKeyDown = createKeybind({
		'$mod+R': () => {
			location.reload();
		}
	});
</script>

<svelte:window on:keydown={handleKeyDown} />
