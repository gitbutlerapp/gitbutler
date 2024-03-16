<script lang="ts">
	import '../styles/main.postcss';

	import { ProjectService } from '$lib/backend/projects';
	import { UpdaterService } from '$lib/backend/updater';
	import AppUpdater from '$lib/components/AppUpdater.svelte';
	import ShareIssueModal from '$lib/components/ShareIssueModal.svelte';
	import { GitHubService } from '$lib/github/service';
	import ToastController from '$lib/notifications/ToastController.svelte';
	import { SETTINGS_CONTEXT, loadUserSettings } from '$lib/settings/userSettings';
	import { UserService } from '$lib/stores/user';
	import * as events from '$lib/utils/events';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/random';
	import { initTheme } from '$lib/utils/theme';
	import { onMount, setContext } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	export let data: LayoutData;
	const { cloud } = data;

	const userSettings = loadUserSettings();
	initTheme(userSettings);
	setContext(SETTINGS_CONTEXT, userSettings);

	$: setContext(UserService, data.userService);
	$: setContext(ProjectService, data.projectService);
	$: setContext(UpdaterService, data.updaterService);
	$: setContext(GitHubService, data.githubService);

	let shareIssueModal: ShareIssueModal;

	$: zoom = $userSettings.zoom || 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';
	$: userSettings.update((s) => ({ ...s, zoom: zoom }));

	onMount(() =>
		unsubscribe(
			events.on('goto', (path: string) => goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show()),

			// Zoom using cmd +, - and =
			hotkeys.on('$mod+Equal', () => (zoom = Math.min(zoom + 0.0625, 3))),
			hotkeys.on('$mod+Minus', () => (zoom = Math.max(zoom - 0.0625, 0.375))),
			hotkeys.on('$mod+Digit0', () => (zoom = 1)),
			hotkeys.on('Meta+T', () => {
				userSettings.update((s) => ({
					...s,
					theme: $userSettings.theme == 'light' ? 'dark' : 'light'
				}));
			})
		)
	);
</script>

<div data-tauri-drag-region class="app-root">
	<slot />
</div>
<Toaster />
<ShareIssueModal bind:this={shareIssueModal} {cloud} />
<ToastController />
<AppUpdater />

<style lang="postcss">
	.app-root {
		display: flex;
		height: 100%;
		user-select: none;
		cursor: default;
	}
</style>
