<script lang="ts" generics="A">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Snippet } from 'svelte';

	type Result<A> = {
		data?: A;
		status: QueryStatus;
		error?: unknown;
	};

	type Props<A> = {
		result: Result<A>;
		children: Snippet<[A]>;
		empty?: Snippet;
	};

	// eslint-disable-next-line no-undef
	const { result, children }: Props<A> = $props();
	const { data, status, error } = $derived(result);
</script>

{#if status === 'fulfilled'}
	<!-- Show empty message if data is an empty array. -->
	{#if data !== undefined}
		{@render children(data)}
	{/if}
{:else if status === 'pending'}
	<div class="loading-spinner">
		<Icon name="spinner" />
	</div>
{:else if status === 'rejected'}
	{String(error)}
{:else if status === 'uninitialized'}
	Uninitialized...
{/if}

<style>
	.loading-spinner {
		position: absolute;
		left: 50%;
		transform: translateX(-50%);
		z-index: var(--z-lifted);
		margin-top: 24px;
	}
</style>
