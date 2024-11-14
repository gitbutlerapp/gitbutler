<script lang="ts">
	import type { PatchSeries } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		currentSeries: PatchSeries;
		children: Snippet;
	}

	const { currentSeries, children }: Props = $props();
</script>

<div class="branch-group" data-series-name={currentSeries.name}>
	{@render children()}
</div>

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		scroll-margin-top: 120px;

		&:last-child {
			margin-bottom: 12px;
		}
	}
</style>
