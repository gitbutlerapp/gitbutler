<script lang="ts">
	import '../app.postcss';

	import { open } from '@tauri-apps/api/dialog';
	import { toasts, Toaster, events, hotkeys, stores } from '$lib';
	import type { LayoutData } from './$types';
	import { Link, CommandPalette } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from '@square/svelte-store';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils';
	import LinkProjectModal from './LinkProjectModal.svelte';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import ShareIssueModal from './ShareIssueModal.svelte';

	export let data: LayoutData;
	const { posthog, projects, sentry, cloud } = data;

	const user = stores.user;

	const project = derived([page, projects], ([page, projects]) =>
		projects?.find((project) => project.id === page.params.projectId)
	);

	let commandPalette: CommandPalette;
	let linkProjectModal: LinkProjectModal;
	let shareIssueModal: ShareIssueModal;

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
			events.on('openCommandPalette', () => commandPalette?.show()),
			events.on('closeCommandPalette', () => commandPalette?.close()),
			events.on('goto', (path: string) => goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show()),

			hotkeys.on('Meta+k', () => events.emit('openCommandPalette')),
			hotkeys.on('Meta+,', () => events.emit('goto', '/users/')),
			hotkeys.on('Meta+Shift+N', () => events.emit('openNewProjectModal')),

			user.subscribe(posthog.identify),
			user.subscribe(sentry.identify)
		)
	);
</script>

<div class="flex h-full max-h-full min-h-full flex-col">
	<header
		data-tauri-drag-region
		class="flex h-11 flex-row items-center border-b border-zinc-700 pt-1 pb-1 text-zinc-400"
		style="z-index: 9999;"
	>
		<div class="breadcrumb-project-container ml-24">
			<Breadcrumbs project={$project} />
		</div>
		<div class="flex-grow" />
		<div class="mr-6">
			<Link href="/users/">
				{#await user.load() then}
					{#if $user !== null}
						{#if $user.picture}
							<img class="inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
						{/if}
						<span class="hover:no-underline">{$user.name}</span>
					{:else}
						<span>Connect to GitButler Cloud</span>
					{/if}
				{/await}
			</Link>
		</div>
	</header>

	<div class="flex-grow overflow-auto">
		<slot />
	</div>

	<Toaster />

	{#await Promise.all([projects.load(), project.load()]) then}
		<CommandPalette bind:this={commandPalette} {projects} {project} />
	{/await}

	<LinkProjectModal bind:this={linkProjectModal} {cloud} {projects} />

	<ShareIssueModal bind:this={shareIssueModal} user={$user} {cloud} />
</div>
