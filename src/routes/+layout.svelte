<script lang="ts">
	import '../app.postcss';

	import { open } from '@tauri-apps/api/dialog';
	import { toasts, Toaster, events, hotkeys, stores } from '$lib';
	import type { LayoutData } from './$types';
	import { Link } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from '@square/svelte-store';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils';
	import LinkProjectModal from './LinkProjectModal.svelte';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import ShareIssueModal from './ShareIssueModal.svelte';
	import { initTheme } from '$lib/theme';
	import ThemeSelector from './ThemeSelector.svelte';

	initTheme();

	export let data: LayoutData;
	const { posthog, projects, sentry, cloud } = data;

	const user = stores.user;

	const project = derived([page, projects], ([page, projects]) =>
		projects?.find((project) => project.id === page.params.projectId)
	);

	let linkProjectModal: LinkProjectModal;
	let shareIssueModal: ShareIssueModal;

	let zoom = 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';

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
		<ThemeSelector />
		<div class="mr-6">
			{#await user.load() then}
				<Link href="/user/">
					{#if $user !== null}
						{#if $user.picture}
							<img class="inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
						{/if}
						<span class="hover:no-underline">{$user.name}</span>
					{:else}
						Account
					{/if}
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
</div>
