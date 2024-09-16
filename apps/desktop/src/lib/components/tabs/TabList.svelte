<script lang="ts">
	import { TabStyle, type TabContext } from './types';
	import { getContext, type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const tabStore = getContext<TabContext>('tab');
</script>

<ul class="tab-list" class:segment-control={tabStore.style === TabStyle.SegmentControl}>
	{@render children()}
</ul>

<style>
	.tab-list {
		width: 100%;
		display: flex;

		&.segment-control {
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
		}
	}
</style>
