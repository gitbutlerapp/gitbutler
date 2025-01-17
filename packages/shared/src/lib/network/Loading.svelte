<script lang="ts" generics="A">
	import LoadingState from './LoadingState.svelte';
	import type { Loadable } from '$lib/network/types';
	import type { Snippet } from 'svelte';

	type Props<A> = {
		loadable?: Loadable<A>;
		children: Snippet<[A]>;
	};

	// eslint-disable-next-line no-undef
	const { loadable, children }: Props<A> = $props();
</script>

{#if !loadable}
	<p>Uninitialized...</p>
{:else if loadable.status === 'found'}
	{@render children(loadable.value)}
{:else if loadable.status === 'loading'}
	<LoadingState />
{:else if loadable.status === 'not-found'}
	<p>Not found</p>
{:else if loadable.status === 'error'}
	<p>{loadable.error.message}</p>
{:else}
	<p>Unknown state</p>
{/if}
