<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { onMount } from 'svelte';
	import type { CommitStatus } from '$lib/vbranches/types';

	export let isExpandable = true;
	export let expanded: boolean;
	export let type: CommitStatus;
	export let height: number | undefined;
	export let commitCount = 0;

	let element: HTMLButtonElement | undefined = undefined;

	onMount(() => (height = element?.offsetHeight));
</script>

<button class="header" bind:this={element} on:click={() => (expanded = !expanded)}>
	<div class="title text-base-13 text-semibold">
		{#if type == 'local'}
			Local
		{:else if type == 'remote'}
			Remote branch
		{:else if type == 'integrated'}
			Integrated
		{:else if type == 'upstream'}
			{commitCount} upstream {commitCount == 1 ? 'commit' : 'commits'}
			<Icon name="warning" color="warning" />
		{/if}
	</div>
	{#if isExpandable}
		<div class="expander">
			<Icon name={expanded ? 'chevron-down' : 'chevron-top'} />
		</div>
	{/if}
</button>

<style lang="postcss">
	.header {
		display: flex;
		align-items: center;
		padding: var(--size-16) var(--size-14) var(--size-16) var(--size-14);
		justify-content: space-between;
		gap: var(--size-8);

		&:hover {
			& .expander {
				opacity: 1;
			}
		}
	}
	.title {
		display: flex;
		align-items: center;
		color: var(--clr-scale-ntrl-0);
		gap: var(--size-8);
		overflow-x: hidden;
	}

	.expander {
		color: var(--clr-scale-ntrl-50);
		opacity: 0.5;
		transition: opacity var(--transition-fast);
	}
</style>
