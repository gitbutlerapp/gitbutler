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

	type BaseProps<A> = {
		type: 'stack' | 'project' | 'optional-stack';
		result: Result<A> | undefined;
		empty?: Snippet;
		projectId: string;
	};

	/**
	 * Children depend on a the result of a RTK query, project ID and stack ID.
	 */
	type StackProps<A> = BaseProps<A> & {
		type: 'stack';
		stackId: string;
		children: Snippet<[A, { stackId: string; projectId: string }]>;
	};

	/**
	 * Children depend on a the result of a RTK query, project ID and an optional stack ID.
	 *
	 * @note Only use this if the stack ID is optional. If the stack ID is required, use the props of type `stack`.
	 */
	type OptionalStackProps<A> = BaseProps<A> & {
		type: 'optional-stack';
		stackId: string | undefined;
		children: Snippet<[A, { stackId: string | undefined; projectId: string }]>;
	};

	/**
	 * Children depend on a the result of a RTK query and project ID.
	 */
	type ProjectProps<A> = BaseProps<A> & {
		type: 'project';
		children: Snippet<[A, { projectId: string }]>;
	};

	type Props<A> = StackProps<A> | ProjectProps<A> | OptionalStackProps<A>;

	let props: Props<A> = $props();

	type CachedData =
		| {
				type: 'stack';
				data: A;
				stackId: string;
				projectId: string;
		  }
		| {
				type: 'project';
				data: A;
				projectId: string;
		  }
		| {
				type: 'optional-stack';
				data: A;
				stackId: string | undefined;
				projectId: string;
		  };

	/**
	 * To prevent flickering when the input data changes for an already
	 * rendered component we render the previous result until the next
	 * result is ready.
	 */
	let cachedData = $state<CachedData | undefined>(undefined);

	function setCachedData(data: A): CachedData {
		switch (props.type) {
			case 'stack': {
				return {
					type: 'stack',
					data: data,
					stackId: props.stackId,
					projectId: props.projectId
				};
			}

			case 'project': {
				return {
					type: 'project',
					data: data,
					projectId: props.projectId
				};
			}

			case 'optional-stack': {
				return {
					type: 'optional-stack',
					data: data,
					stackId: props.stackId,
					projectId: props.projectId
				};
			}
		}
	}

	$effect(() => {
		if (props.result?.data !== undefined) {
			cachedData = setCachedData(props.result.data);
		}
	});
</script>

{#if cachedData !== undefined}
	{#if cachedData.type === 'stack' && props.type === 'stack'}
		{@render props.children(cachedData.data, {
			stackId: cachedData.stackId,
			projectId: cachedData.projectId
		})}
	{:else if cachedData.type === 'project' && props.type === 'project'}
		{@render props.children(cachedData.data, {
			projectId: cachedData.projectId
		})}
	{:else if cachedData.type === 'optional-stack' && props.type === 'optional-stack'}
		{@render props.children(cachedData.data, {
			stackId: cachedData.stackId,
			projectId: cachedData.projectId
		})}
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
