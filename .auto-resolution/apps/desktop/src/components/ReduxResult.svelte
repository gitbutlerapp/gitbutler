<script lang="ts" module>
	type A = unknown;
</script>

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

	const { result, children }: Props<A> = $props();

	let dataCopy: undefined | Result<A>['data'] = $state(result?.data);

	/**
	 * To prevent flickering when the input data changes for an already
	 * rendered component we render the previous result until the next
	 * result is ready.
	 */
	$effect(() => {
		if (result?.data) dataCopy = result.data;
	});
</script>

{#if dataCopy !== undefined}
	{@render children(dataCopy)}
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
		z-index: var(--z-lifted);
		position: relative;
	}
</style>
