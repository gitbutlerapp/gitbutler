<script lang="ts">
	import '../app.postcss';

	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { BackForwardButtons, Button } from '$lib/components';
	import Breadcrumbs from '$lib/components/Breadcrumbs.svelte';
	import { page } from '$app/stores';
	import CommandPalette from '$lib/components/CommandPalette/CommandPalette.svelte';
	import { readable } from 'svelte/store';

	export let data: LayoutData;
	const { user, posthog, projects } = data;

	$: project = $page.params.projectId ? projects.get($page.params.projectId) : readable(undefined);

	user.subscribe(posthog.identify);
</script>

<div class="flex h-full max-h-full min-h-full flex-col">
	<header
		data-tauri-drag-region
		class="flex select-none flex-row items-center border-b border-zinc-700 pt-1 pb-1 text-zinc-400"
	>
		<div class="ml-24">
			<BackForwardButtons />
		</div>
		<div class="ml-6">
			<Breadcrumbs {project} />
		</div>
		<div class="flex-grow" />
		<div class="mr-6">
			<Button filled={false} href="/users/">
				{#if $user}
					{#if $user.picture}
						<img class="inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
					{/if}
					<span class="hover:no-underline">{$user.name}</span>
				{:else}
					<span>Connect to GitButler Cloud</span>
				{/if}
			</Button>
		</div>
	</header>

	<div class="flex-auto overflow-auto">
		<slot />
	</div>
	<Toaster />
	<CommandPalette {projects} {project} />
</div>
