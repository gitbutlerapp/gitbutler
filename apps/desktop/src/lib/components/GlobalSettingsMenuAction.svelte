<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	onMount(() => {
		const unsubscribeSettings = listen<string>('menu://global/settings/clicked', () => {
			if (!window.location.pathname.startsWith('/settings/')) {
				goto(`/settings/`);
			}
		});

		return () => {
			unsubscribeSettings();
		};
	});
</script>
