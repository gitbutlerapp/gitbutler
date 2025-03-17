<script lang="ts" generics="A">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Snippet } from 'svelte';

	type Result<A> = {
		data?: A;
		status: QueryStatus;
		error?: unknown;
	};

	type Props<A> = {
		result: Result<A> | undefined;
		children: Snippet<[A]>;
		empty?: Snippet;
	};

	// eslint-disable-next-line no-undef
	const { result, children }: Props<A> = $props();
</script>

{#if result?.status === 'fulfilled'}
	<!-- Show empty message if data is an empty array. -->
	{#if result.data !== undefined}
		{@render children(result.data)}
	{/if}
{:else if result?.status === 'pending'}
	<div class="loading-spinner">
		<Icon name="spinner" />
	</div>
{:else if result?.status === 'rejected'}
	{#if isErrorlike(result.error)}
		{result.error.message}
	{:else}
		{String(result.error)}
	{/if}
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
