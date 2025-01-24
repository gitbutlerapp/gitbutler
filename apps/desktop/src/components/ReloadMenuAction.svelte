<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { onMount } from 'svelte';

	onMount(() => {
		const unsubscribe = listen<string>('menu://view/reload/clicked', () => {
			location.reload();
		});

		return async () => {
			unsubscribe();
		};
	});
</script>
