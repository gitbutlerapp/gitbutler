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
	<span>Uninitialized...</span>
{:else if loadable.status === 'found'}
	{@render children(loadable.value)}
{:else if loadable.status === 'loading'}
	<LoadingState />
{:else if loadable.status === 'not-found'}
	<span>Not found</span>
{:else if loadable.status === 'error'}
	<span>{loadable.error.message}</span>
{:else}
	<span>Unknown state</span>
{/if}
