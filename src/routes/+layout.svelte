<script lang="ts">
	import '../styles/main.postcss';

	import { open } from '@tauri-apps/api/dialog';
	import * as toasts from '$lib/toasts';
	import * as hotkeys from '$lib/hotkeys';
	import * as events from '$lib/events';
	import { Toaster } from 'svelte-french-toast';
	import { userStore } from '$lib/stores/user';
	import type { LayoutData } from './$types';
	import { Link, Tooltip } from '$lib/components';
	import { IconEmail } from '$lib/icons';
	import { page } from '$app/stores';
	import { derived } from '@square/svelte-store';
	import { onMount, setContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils';
	import LinkProjectModal from './LinkProjectModal.svelte';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import ShareIssueModal from './ShareIssueModal.svelte';
	import { SETTINGS_CONTEXT, loadUserSettings } from '$lib/userSettings';
	import { initTheme } from '$lib/theme';

	export let data: LayoutData;
	const { posthog, projects, sentry, cloud } = data;

	const user = userStore;

	const userSettings = loadUserSettings();
	initTheme(userSettings);
	setContext(SETTINGS_CONTEXT, userSettings);

	const project = derived([page, projects], ([page, projects]) =>
		projects?.find((project) => project.id === page.params.projectId)
	);

	let linkProjectModal: LinkProjectModal;
	let shareIssueModal: ShareIssueModal;

	$: zoom = $userSettings.zoom || 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';
	$: userSettings.update((s) => ({ ...s, zoom: zoom }));

	onMount(() =>
		unsubscribe(
			events.on('openNewProjectModal', () =>
				open({ directory: true, recursive: true })
					.then((selectedPath) => {
						if (selectedPath === null) return;
						if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
						const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
						return projects.add({ path: projectPath });
					})
					.then(async (project) => {
						if (!project) return;
						toasts.success(`Project ${project.title} created`);
						linkProjectModal?.show(project.id);
						goto(`/repo/${project.id}/`);
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
			}),

			user.subscribe(posthog.identify),
			user.subscribe(sentry.identify)
		)
	);
</script>

<div class="flex h-full flex-col">
	<header
		data-tauri-drag-region
		class="flex h-8 flex-shrink-0 flex-row items-center gap-x-6 border-b border-light-300 bg-white text-light-900 dark:border-dark-500 dark:bg-dark-900 dark:text-dark-100"
		style="z-index: 9999;"
	>
		<div class="breadcrumb-project-container ml-[80px]">
			<Breadcrumbs project={$project} />
		</div>
		<div class="flex-grow" />
		<div class="mr-6">
			{#await user.load() then}
				<Link href="/user/">
					{#if $user?.picture}
						<img class="mr-1 inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
					{/if}
					{$user?.name ?? 'Account'}
				</Link>
			{/await}
		</div>
	</header>

	<div class="flex flex-grow overflow-y-auto">
		<slot />
	</div>

	<Toaster />

	<LinkProjectModal bind:this={linkProjectModal} {cloud} {projects} />

	<ShareIssueModal bind:this={shareIssueModal} user={$user} {cloud} />
	<footer class="w-full text-sm font-medium text-light-700 dark:text-dark-100">
		<div
			class="flex h-[1.375rem] flex-shrink-0 select-none items-center border-t border-light-300 bg-white dark:border-dark-500 dark:bg-dark-900"
		>
			<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2 pb-[1px]">
				<div>
					{#if $project}
						<Link href="/repo/{$project?.id}/settings">
							<div class="flex flex-row items-center space-x-2">
								{#if $project?.api?.sync}
									<div class="h-2 w-2 rounded-full bg-green-700" />
									<span>online</span>
								{:else}
									<div class="h-2 w-2 rounded-full bg-red-700" />
									<span class="text-light-600 dark:text-dark-200">offline</span>
								{/if}
							</div>
						</Link>
					{/if}
				</div>

				<div class="flex items-center gap-1">
					<Tooltip label="Send feedback">
						<IconEmail class="h-4 w-4" on:click={() => events.emit('openSendIssueModal')} />
					</Tooltip>
				</div>
			</div>
		</div>
	</footer>
</div>
