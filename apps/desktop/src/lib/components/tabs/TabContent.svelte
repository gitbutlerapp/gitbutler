<script lang="ts">
	import { type Snippet, getContext } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { TabContext } from './types';

	interface Props {
		children: Snippet;
		value: string;
	}

	const { children, value }: Props = $props();

	const tabStore = getContext<TabContext>('tab');
	const selectedIndex = $derived(tabStore.selectedIndex);
	const isActive = $derived($selectedIndex === value);
</script>

{#if isActive}
	<div data-value={value} class="tab-content" in:fade={{ duration: 200 }}>
		{@render children()}
	</div>
{/if}

<style>
	.tab-content {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		justify-content: flex-start;
		margin-top: 1rem;
	}
</style>
