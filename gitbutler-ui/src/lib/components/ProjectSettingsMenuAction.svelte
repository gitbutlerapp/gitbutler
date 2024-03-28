<script lang="ts">
	import { invoke, listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { getContextByClass } from '$lib/utils/context';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const project = getContextByClass(Project);

	function setProjectSettingsEnabled(enabled: boolean) {
		return invoke('menu_item_set_enabled', {
			menuItemId: 'project/settings',
			enabled
		});
	}

	onMount(() => {
		setProjectSettingsEnabled(true);

		const projectSettingsSubscription = listen<string>('menu://project/settings/clicked', () => {
			goto(`/${project.id}/settings/`);
		});

		return () => {
			unsubscribe(projectSettingsSubscription)();
			setProjectSettingsEnabled(false);
		};
	});
</script>
