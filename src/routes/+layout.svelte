<script lang="ts">
	import '../app.postcss';

	import { Toaster } from '$lib';
	import type { LayoutData } from './$types';
	import {
		BackForwardButtons,
		Link,
		CommandPalette,
		Breadcrumbs,
		OpenNewProjectModal
	} from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from '@square/svelte-store';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils';

	export let data: LayoutData;
	const { user, posthog, projects, sentry, events, hotkeys } = data;

	const project = derived([page, projects], ([page, projects]) =>
		projects?.find((project) => project.id === page.params.projectId)
	);

	let commandPalette: CommandPalette;
	let openNewProjectModal: OpenNewProjectModal;

	onMount(() =>
		unsubscribe(
			events.on('openNewProjectModal', () => openNewProjectModal?.show()),
			events.on('openCommandPalette', () => commandPalette?.show()),
			events.on('closeCommandPalette', () => commandPalette?.close()),
			events.on('goto', (path: string) => goto(path)),

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
		class="flex flex-row items-center border-b border-zinc-700 pt-1 pb-1 text-zinc-400"
	>
		<div class="breadkcrumb-back-forward-container ml-24">
			<BackForwardButtons />
		</div>
		<div class="breadcrumb-project-container ml-6">
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

	<div class="flex-auto overflow-auto">
		<slot />
	</div>
	<Toaster />
	{#await Promise.all([projects.load(), project.load()]) then}
		<CommandPalette bind:this={commandPalette} {projects} {project} {events} />
	{/await}
</div>

<OpenNewProjectModal bind:this={openNewProjectModal} {projects} />
