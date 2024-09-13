<script lang="ts">
	import { getContext, type Snippet } from 'svelte';
	import type { TabContext } from './types';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const tabContext = getContext<TabContext>('tab');
</script>

<ul class="tab-list">
	{@render children()}
</ul>

<style>
	.tab-list {
		display: flex;
		gap: 0.5rem;
	}
</style>
