<script lang="ts">
	import { goto } from '$app/navigation';
	import { showHistoryView } from '$lib/config/config';
	import { vscodePath } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { projectSettingsPath } from '$lib/routes/routes.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const { projectId }: { projectId: string } = $props();

	const projectsService = getContext(ProjectsService);

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const shortcutService = getContext(ShortcutService);

	shortcutService.on('project-settings', () => {
		goto(projectSettingsPath(projectId));
	});

	shortcutService.on('open-in-vscode', async () => {
		const result = await projectsService.fetchProject(projectId);
		const project = result.data;
		if (!project) {
			throw new Error(`Project not found: ${projectId}`);
		}
		openExternalUrl(
			getEditorUri({
				schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
				path: [vscodePath(project.path)],
				searchParams: { windowId: '_blank' }
			})
		);
	});

	shortcutService.on('history', () => {
		$showHistoryView = !$showHistoryView;
	});

	const unsubscribeHistoryButton = unsubscribe(
		events.on('openHistory', () => {
			$showHistoryView = true;
		})
	);

	onMount(() => {
		return () => {
			unsubscribeHistoryButton();
		};
	});
</script>
