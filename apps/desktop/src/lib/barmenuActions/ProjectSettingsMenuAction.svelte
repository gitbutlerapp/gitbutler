<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { showHistoryView } from '$lib/config/config';
	import { editor } from '$lib/editorLink/editorLink';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
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
				const path = `${$editor}://file${project.vscodePath}?windowId=_blank`;
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
