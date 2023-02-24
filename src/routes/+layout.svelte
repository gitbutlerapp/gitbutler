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

<header
	data-tauri-drag-region
	class="sticky top-0 z-50 flex flex-row items-center h-8 overflow-hidden border-b select-none  text-zinc-400 border-zinc-700 bg-zinc-900 "
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

<div class="h-0 min-h-full bg-zinc-800 text-zinc-400">
	<slot />

	<Toaster />
</div>
