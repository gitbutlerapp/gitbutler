<script lang="ts" module>
	type A = unknown;
	type B = string | undefined;
</script>

<script lang="ts" generics="A, B extends string | undefined">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { isDefined, isErrorlike } from '@gitbutler/ui/utils/typeguards';
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

	type Props<A, B extends string | undefined> = {
		result: Result<A> | undefined;
		projectId: string;
		children: Snippet<[A, Env<B>]>;
	} & (B extends undefined ? { stackId?: B } : { stackId: B });

	const props: Props<A, B> = $props();

	type Display = {
		result: Result<A> | undefined;
		env: Env<B>;
	};

	let cache: Display | undefined;
	const display = $derived.by<Display>(() => {
		const env = { projectId: props.projectId, stackId: props.stackId as B };
		if (isDefined(props.result?.data)) {
			cache = { result: props.result, env };
			return cache;
		} else {
			if (cache) {
				return cache;
			} else {
				return { result: props.result, env };
			}
		}
	});
</script>

{#if display.result?.data !== undefined}
	{@render props.children(display.result.data, display.env)}
{:else if display.result?.status === 'pending'}
	<div class="loading-spinner">
		<Icon name="spinner" />
	</div>
{:else if display.result?.status === 'rejected'}
	{#if isErrorlike(display.result.error)}
		{display.result.error.message}
	{:else}
		{JSON.stringify(display.result.error)}
	{/if}
{:else if display.result?.status === 'uninitialized'}
	Uninitialized...
{/if}

<style>
	.loading-spinner {
		z-index: var(--z-lifted);
		position: relative;
	}
</style>
