<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { showHistoryView } from '$lib/config/config';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import type { Writable } from 'svelte/store';

	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	onMount(() => {
		const unsubscribeSettings = listen<string>('menu://project/settings/clicked', () => {
			goto(`/${project.id}/settings/`);
		});

		const unsubscribeOpenInVSCode = listen<string>(
			'menu://project/open-in-vscode/clicked',
			async () => {
				const path = `${$userSettings.defaultCodeEditor}://file${project.vscodePath}?windowId=_blank`;
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
			unsubscribeOpenInVSCode();
			unsubscribeHistory();
			unsubscribeHistoryButton();
		};
	});
</script>
