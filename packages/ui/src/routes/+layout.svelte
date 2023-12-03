<script lang="ts">
	import '../styles/main.postcss';

	import { homeDir } from '@tauri-apps/api/path';
	import { open } from '@tauri-apps/api/dialog';
	import * as toasts from '$lib/utils/toasts';
	import * as hotkeys from '$lib/utils/hotkeys';
	import * as events from '$lib/utils/events';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { onMount, setContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils/random';
	import LinkProjectModal from './LinkProjectModal.svelte';
	import ShareIssueModal from './ShareIssueModal.svelte';
	import { SETTINGS_CONTEXT, loadUserSettings } from '$lib/settings/userSettings';
	import { initTheme } from './user/theme';

	export let data: LayoutData;
	const { projectService, cloud, user$ } = data;

	const userSettings = loadUserSettings();
	initTheme(userSettings);
	setContext(SETTINGS_CONTEXT, userSettings);

	let linkProjectModal: LinkProjectModal;
	let shareIssueModal: ShareIssueModal;

	$: zoom = $userSettings.zoom || 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';
	$: userSettings.update((s) => ({ ...s, zoom: zoom }));

	onMount(() =>
		unsubscribe(
			events.on('openNewProjectModal', async () =>
				open({ directory: true, recursive: true, defaultPath: await homeDir() })
					.then((selectedPath) => {
						if (selectedPath === null) return;
						if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
						const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
						return projectService.add(projectPath);
					})
					.then(async (project) => {
						if (!project) return;
						toasts.success(`Project ${project.title} created`);
						// linkProjectModal?.show(project.id);
						goto(`/${project.id}/board`);
					})
					.catch((e: any) => toasts.error(e.message))
			),
			events.on('goto', (path: string) => goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show()),

			hotkeys.on('Meta+Shift+N', () => events.emit('openNewProjectModal')),

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

<div class="flex h-full flex-col">
	<div class="flex flex-grow justify-center overflow-hidden">
		<slot />
	</div>
	<Toaster />
	<LinkProjectModal bind:this={linkProjectModal} {cloud} {projectService} user={$user$} />
	<ShareIssueModal bind:this={shareIssueModal} user={$user$} {cloud} />
</div>
