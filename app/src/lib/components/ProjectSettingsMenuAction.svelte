<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import { editor } from '$lib/utils/systemEditor';
	import { open } from '@tauri-apps/api/shell';
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
				const path = `${editor.get()}://file${project.vscodePath}?windowId=_blank`;
				open(path);
			}
		);

		return () => {
			unsubscribeSettings();
			unsubscribeOpenInVSCode();
		};
	});
</script>
