<script lang="ts">
	import '../app.postcss';

	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { BackForwardButtons } from '$lib/components';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import Breadcrumbs from '$lib/components/Breadcrumbs.svelte';

	export let data: LayoutData;
	const { user, posthog } = data;

	setContext('project', writable(null));
	setContext('session', writable(null));

	user.subscribe(posthog.identify);
</script>

<div class="flex flex-col min-h-full max-h-full h-full bg-zinc-800 text-zinc-400">
	<header
		data-tauri-drag-region
		class="flex flex-row items-center h-8 border-b select-none  text-zinc-400 border-zinc-700 bg-zinc-900 "
	>
		<div class="ml-24">
			<BackForwardButtons />
		</div>
		<div class="ml-6"><Breadcrumbs /></div>
		<div class="flex-grow" />
		<a href="/users/" class="flex items-center gap-2 mr-4 font-medium hover:text-zinc-200">
			{#if $user}
				{#if $user.picture}
					<img class="inline-block w-6 h-6 rounded-full" src={$user.picture} alt="Avatar" />
				{/if}
				<span>{$user.name}</span>
			{:else}
				<span>Connect to GitButler Cloud</span>
			{/if}
		</a>
	</header>

	<div class="flex-auto overflow-auto bg-zinc-900 text-zinc-400">
		<slot />
	</div>
	<Toaster />
</div>
