<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { open } from '@tauri-apps/api/shell';
	import { invoke } from '@tauri-apps/api/tauri';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const project = getContext(Project);

	onMount(() => {
		const unsubscribeSettings = listen<string>('menu://project/settings/clicked', () => {
			goto(`/${project.id}/settings/`);
		});

		const unsubscribeOpenInVSCode = listen<string>(
			'menu://project/open-in-vscode/clicked',
			async () => {
				const editor = await invoke('resolve_vscode_variant');
				const path = `${editor}://file${project.path}`;
				open(path);
			}
		);

		return () => {
			unsubscribeSettings();
			unsubscribeOpenInVSCode();
		};
	});
</script>
