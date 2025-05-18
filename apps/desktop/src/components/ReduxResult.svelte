<script lang="ts" module>
	type A = unknown;
	type B = string | undefined;
</script>

<script lang="ts" generics="A, B extends string | undefined">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { isParsedError } from '$lib/error/parser';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
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
		loading?: Snippet<[]>;
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
		} else if (isDefined(props.result?.data)) {
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

	$effect(() => {
		if (props.onerror && display.result?.error !== undefined) {
			props.onerror(display.result.error);
		}
	});
</script>

{#if display.result?.error}
	{@const error = display.result.error}
	{#if isParsedError(error)}
		<InfoMessage error={error.message} style="error">
			{#snippet title()}
				{error.name}
			{/snippet}
			{#snippet content()}
				An asynchronous operation failed.
			{/snippet}
		</InfoMessage>
	{/if}
	{#if props.error}
		{@render props.error(display.result.error)}
	{/if}
{:else if display.result?.status === 'pending' || display.result?.status === 'uninitialized'}
	{#if props.loading}
		{@render props.loading()}
	{:else}
		<div class="text-12 loading-spinner">
			<Icon name="spinner" />
			<span>{display.result?.status}</span>
		</div>
	{/if}
{:else if display.result?.data !== undefined}
	{@render props.children(display.result.data, display.env)}
{/if}

<style>
	.loading-spinner {
		display: flex;
		align-items: center;
		gap: 8px;
		z-index: var(--z-lifted);
		position: relative;
		color: var(--clr-text-2);
	}
</style>
