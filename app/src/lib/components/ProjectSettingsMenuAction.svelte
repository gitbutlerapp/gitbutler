<script lang="ts">
	import { invoke, listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const project = getContext(Project);

	async function setEnabled(enabled: boolean) {
		return invoke('menu_item_set_enabled', {
			menuItemId: 'project/settings',
			enabled
		});
	}

	onMount(() => {
		setEnabled(true);

		const unsubscribe = listen<string>('menu://project/settings/clicked', () => {
			goto(`/${project.id}/settings/`);
		});

		return () => {
			unsubscribe();
			setEnabled(false);
		};
	});
</script>
