<script lang="ts" generics="A">
	import type { LoadableData } from '$lib/network/types';
	import type { Snippet } from 'svelte';

	type Props<A> = {
		loadable?: LoadableData<A, unknown>;
		children: Snippet<[A]>;
	};

	// eslint-disable-next-line no-undef
	const { loadable, children }: Props<A> = $props();
</script>

{#if !loadable}
	<p>Uninitialized...</p>
{:else if loadable.type === 'found'}
	{@render children(loadable.value)}
{:else if loadable.type === 'loading'}
	<p>Loading...</p>
{:else if loadable.type === 'not-found'}
	<p>Not found</p>
{:else if loadable.type === 'error'}
	<p>{loadable.error.message}</p>
{:else}
	<p>Unknown state</p>
{/if}
