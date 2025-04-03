<script lang="ts" module>
	type A = unknown;
	type B = string | undefined;
</script>

<script lang="ts" generics="A, B extends string | undefined">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Snippet } from 'svelte';

	type Result<A> = {
		data?: A;
		status: QueryStatus;
		error?: unknown;
	};

	type Env<B> = {
		projectId: string;
		stackId: B;
	};

	type Props<A, B> = {
		result: Result<A> | undefined;
		empty?: Snippet;
		projectId: string;
		stackId?: B;
		children: Snippet<[A, Env<B>]>;
	};

	let props: Props<A, B> = $props();

	type CachedData<A, B> = {
		data: A;
		projectId: string;
		stackId: Props<A, B>['stackId'];
	};

	/**
	 * To prevent flickering when the input data changes for an already
	 * rendered component we render the previous result until the next
	 * result is ready.
	 */
	let cachedData = $state<CachedData<A, B> | undefined>(undefined);

	function setCachedData(data: A): CachedData<A, B> {
		return {
			data,
			projectId: props.projectId,
			stackId: props.stackId
		};
	}

	$effect(() => {
		if (props.result?.data !== undefined) {
			cachedData = setCachedData(props.result.data);
		}
	});
</script>

{#if cachedData !== undefined}
	{@render props.children(cachedData.data, {
		projectId: cachedData.projectId,
		stackId: cachedData.stackId as B
	})}

	{#if props.result?.status === 'fulfilled'}
		{#if props.result.data === undefined}
			{props.empty}
		{/if}
	{:else if props.result?.status === 'pending'}
		<div class="loading-spinner">
			<Icon name="spinner" />
		</div>
	{/if}
{:else if props.result?.status === 'pending'}
	<div class="loading-spinner">
		<Icon name="spinner" />
	</div>
{:else if props.result?.status === 'rejected'}
	{#if isErrorlike(props.result.error)}
		{props.result.error.message}
	{:else}
		{JSON.stringify(props.result.error)}
	{/if}
{:else if props.result?.status === 'uninitialized'}
	Uninitialized...
{/if}

<style>
	.loading-spinner {
		z-index: var(--z-lifted);
		position: relative;
	}
</style>
