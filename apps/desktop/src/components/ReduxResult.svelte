<script lang="ts" module>
	type A = unknown;
	type B = string | undefined;
</script>

<script lang="ts" generics="A, B extends string | undefined">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { isParsedError } from '$lib/error/parser';

	import { Icon } from '@gitbutler/ui';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Result } from '$lib/state/helpers';
	import type { Snippet } from 'svelte';

	type Env<B> = {
		projectId: string;
		stackId: B;
	};

	type Props<A, B extends string | undefined> = {
		result: Result<A> | undefined;
		projectId: string;
		children: Snippet<[A, Env<B>]>;
		loading?: Snippet<[A | undefined]>;
		error?: Snippet<[unknown]>;
		onerror?: (err: unknown) => void;
	} & (B extends undefined ? { stackId?: B } : { stackId: B });

	const props: Props<A, B> = $props();

	type Display = {
		result: Result<A> | undefined;
		env: Env<B>;
	};

	let cache: Display | undefined;

	const display = $derived.by<Display>(() => {
		const env = { projectId: props.projectId, stackId: props.stackId as B };
		if (props.result?.error) {
			return { result: props.result, env };
		}

		// This needs to test for 'undefined' specifically, enabling 'null' as a valid data value.
		if (props.result?.data !== undefined) {
			cache = { result: props.result, env };
			return cache;
		}

		if (cache) {
			return cache;
		}

		return { result: props.result, env };
	});

	$effect(() => {
		if (props.onerror && display.result?.error !== undefined) {
			props.onerror(display.result.error);
		}
	});
</script>

{#snippet errorComponent(error: unknown)}
	{#if props.error}
		{@render props.error(error)}
	{:else if isParsedError(error)}
		<InfoMessage error={error.message} style="error">
			{#snippet title()}
				{error.name}
			{/snippet}
			{#snippet content()}
				An asynchronous operation failed.
			{/snippet}
		</InfoMessage>
	{/if}
{/snippet}

{#snippet loadingComponent(data: A | undefined, status: QueryStatus)}
	{#if props.loading}
		{@render props.loading(data)}
	{:else}
		<div class="text-12 loading-spinner">
			<Icon name="spinner" />
			<span>{status}</span>
		</div>
	{/if}
{/snippet}

{#if display.result?.error}
	{@const error = display.result.error}
	{@render errorComponent(error)}
{:else if display.result?.data !== undefined}
	{@render props.children(display.result.data, display.env)}
{:else if display.result?.status === 'pending' || display.result?.status === 'uninitialized'}
	{@render loadingComponent(display.result.data, display.result.status)}
{/if}

<style>
	.loading-spinner {
		display: flex;
		z-index: var(--z-lifted);
		position: relative;
		align-items: center;
		gap: 8px;
		color: var(--clr-text-2);
	}
</style>
