<script lang="ts" generics="A">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Snippet } from 'svelte';

	type Props<A> = {
		status: QueryStatus;
		error?: unknown;
		data?: A;
		children: Snippet<[A]>;
		empty: Snippet;
	};

	// eslint-disable-next-line no-undef
	const { status, data, error, children, empty }: Props<A> = $props();
</script>

{#if status === 'fulfilled'}
	<!-- Show empty message if data is an empty array. -->
	{#if data !== undefined && (!Array.isArray(data) || data.length > 0)}
		{@render children(data)}
	{:else}
		{@render empty()}
	{/if}
{:else if status === 'pending'}
	<Icon name="spinner" />
{:else if status === 'rejected'}
	{String(error)}
{:else if status === 'uninitialized'}
	Uninitialized...
{/if}
