<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { QueryStatus } from '@reduxjs/toolkit/query';
	import type { Snippet } from 'svelte';

	type Props = {
		status: QueryStatus;
		error: unknown;
		children: Snippet;
	};

	const { status, error, children }: Props = $props();
</script>

{#if status === 'fulfilled'}
	{@render children()}
{:else if status === 'pending'}
	<Icon name="spinner" />
{:else if status === 'rejected'}
	{String(error)}
{:else if status === 'uninitialized'}
	Uninitialized...
{/if}
