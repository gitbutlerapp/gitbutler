<script lang="ts">
	import '../styles/main.postcss';

	import AppUpdater from '$lib/components/AppUpdater.svelte';
	import ShareIssueModal from '$lib/components/ShareIssueModal.svelte';
	import ToastController from '$lib/notifications/ToastController.svelte';
	import { SETTINGS_CONTEXT, loadUserSettings } from '$lib/settings/userSettings';
	import * as events from '$lib/utils/events';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/random';
	import { initTheme } from '$lib/utils/theme';
	import { onMount, setContext } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	export let data: LayoutData;
	const { cloud, user$, updaterService } = data;

	const userSettings = loadUserSettings();
	initTheme(userSettings);
	setContext(SETTINGS_CONTEXT, userSettings);

	let shareIssueModal: ShareIssueModal;

	$: zoom = $userSettings.zoom || 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';
	$: userSettings.update((s) => ({ ...s, zoom: zoom }));

	onMount(() =>
		unsubscribe(
			events.on('goto', (path: string) => goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show()),

			// Zoom using cmd +, - and =
			hotkeys.on('Meta+Equal', () => (zoom = Math.min(zoom + 0.0625, 3))),
			hotkeys.on('Meta+Minus', () => (zoom = Math.max(zoom - 0.0625, 0.375))),
			hotkeys.on('Meta+Digit0', () => (zoom = 1)),
			hotkeys.on('Meta+T', () => {
				userSettings.update((s) => ({
					...s,
					theme: $userSettings.theme == 'light' ? 'dark' : 'light'
				}));
			})
		)
	);
</script>

<div data-tauri-drag-region class="flex h-full flex-grow justify-center overflow-hidden">
	<slot />
</div>
<Toaster />
<ShareIssueModal bind:this={shareIssueModal} user={$user$} {cloud} />
<ToastController />
<AppUpdater {updaterService} />
