<script lang="ts">
	import { showHistoryView } from '$lib/config/config';
	import { Project } from '$lib/project/project';
	import { DesktopRoutesService } from '$lib/routes/routes.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const shortcutService = getContext(ShortcutService);
	const routes = getContext(DesktopRoutesService);

	shortcutService.on('project-settings', () => {
		goto(routes.projectSettingsPath(project.id));
	});

	shortcutService.on('open-in-vscode', () => {
		const path = getEditorUri({
			schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
			path: [project.vscodePath],
			searchParams: { windowId: '_blank' }
		});
		openExternalUrl(path);
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
