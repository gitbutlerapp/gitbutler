<script lang="ts">
	import { goto } from '$app/navigation';
	import { showHistoryView } from '$lib/config/config';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { projectSettingsPath } from '$lib/routes/routes.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { getEditorUri, openExternalUrl, showFileInFolder } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);

	const userSettings = inject(SETTINGS);
	const shortcutService = inject(SHORTCUT_SERVICE);

	shortcutService.on('project-settings', () => {
		goto(projectSettingsPath(projectId));
	});

	shortcutService.on('open-in-vscode', async () => {
		const project = await projectsService.fetchProject(projectId);
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

	shortcutService.on('show-in-finder', async () => {
		const project = await projectsService.fetchProject(projectId);
		if (!project) {
			throw new Error(`Project not found: ${projectId}`);
		}
		// Show the project directory in the default file manager (cross-platform)
		await showFileInFolder(project.path);
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
