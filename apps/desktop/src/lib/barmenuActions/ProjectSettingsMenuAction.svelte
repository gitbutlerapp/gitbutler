<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { editor } from '$lib/editorLink/editorLink';
	import { getContext } from '$lib/utils/context';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { openExternalUrl } from '$lib/utils/url';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const project = getContext(Project);

	interface Props {
		showHistory: boolean;
		onHistoryShow: (show: boolean) => void;
	}

	const { showHistory, onHistoryShow }: Props = $props();
	let showHistoryState = $state(showHistory);

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
			showHistoryState = true;
		});

		const unsubscribeHistoryButton = unsubscribe(
			events.on('openHistory', () => {
				showHistoryState = true;
			})
		);

		return () => {
			unsubscribeSettings();
			unsubscribeOpenInVSCode();
			unsubscribeHistory();
			unsubscribeHistoryButton();
		};
	});

	const handleKeyDown = createKeybind({
		'$mod+Shift+H': () => {
			showHistoryState = !showHistoryState;
		}
	});

	$effect(() => {
		onHistoryShow(showHistoryState);
	});

	$effect(() => {
		showHistoryState = showHistory;
	});
</script>

<svelte:window onkeydown={handleKeyDown} />
