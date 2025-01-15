<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { showHistoryView } from '$lib/config/config';
	import { Project } from '$lib/project/project';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
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

	onMount(() => {
		const unsubscribeSettings = listen<string>('menu://project/settings/clicked', () => {
			goto(`/${project.id}/settings/`);
		});

		const unsubscribeopenInEditor = listen<string>(
			'menu://project/open-in-vscode/clicked',
			async () => {
				const path = getEditorUri({
					schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
					path: [project.vscodePath],
					searchParams: { windowId: '_blank' }
				});
				openExternalUrl(path);
			}
		);

		const unsubscribeHistory = listen<string>('menu://project/history/clicked', () => {
			$showHistoryView = true;
		});

		const unsubscribeHistoryButton = unsubscribe(
			events.on('openHistory', () => {
				$showHistoryView = true;
			})
		);

		return () => {
			unsubscribeSettings();
			unsubscribeopenInEditor();
			unsubscribeHistory();
			unsubscribeHistoryButton();
		};
	});
</script>
