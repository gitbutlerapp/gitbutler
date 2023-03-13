<script lang="ts">
	import '../app.postcss';

	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { BackForwardButtons } from '$lib/components';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import Breadcrumbs from '$lib/components/Breadcrumbs.svelte';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import { currentProject } from '$lib/current_project';

	export let data: LayoutData;
	const { user, posthog, projects } = data;

	setContext('project', writable(null));
	setContext('session', writable(null));
	setContext('projects', projects);

	user.subscribe(posthog.identify);
</script>

<div class="flex h-full max-h-full min-h-full flex-col text-zinc-400">
	<header
		data-tauri-drag-region
		class="flex select-none flex-row items-center border-b border-zinc-700 pt-1 pb-1 text-zinc-400"
	>
		<div class="ml-24">
			<BackForwardButtons />
		</div>
		<div class="ml-6"><Breadcrumbs /></div>
		<div class="flex-grow" />
		<a href="/users/" class="mr-4 flex items-center gap-1 font-medium hover:text-zinc-200">
			{#if $user}
				{#if $user.picture}
					<img class="inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
				{/if}
				<span>{$user.name}</span>
			{:else}
				<span>Connect to GitButler Cloud</span>
			{/if}
		</a>
	</header>

	<div class="flex-auto overflow-auto text-zinc-400">
		<slot />
	</div>
	<Toaster />
	<CommandPalette />
</div>
