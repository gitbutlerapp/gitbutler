<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { onMount } from 'svelte';

	onMount(() => {
		const unsubscribe = listen<string>('menu://view/reload/clicked', () => {
			location.reload();
		});

		return async () => {
			unsubscribe();
		};
	});

	const handleKeyDown = createKeybind({
		'$mod+R': () => {
			location.reload();
		}
	});
</script>

<svelte:window onkeydown={handleKeyDown} />
