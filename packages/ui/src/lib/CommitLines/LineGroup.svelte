<script lang="ts">
	import Line from '$lib/CommitLines/Line.svelte';
	import type { LineGroupData } from '$lib/CommitLines/types';

	interface Props {
		lineGroup: LineGroupData;
		showNode?: boolean;
		topHeightPx?: number;
	}

	const { lineGroup, showNode = true, topHeightPx }: Props = $props();
</script>

<div class="line-group">
	{#each lineGroup.lines as line}
		<Line {line} {topHeightPx} {showNode} />
	{/each}
</div>

<style lang="postcss">
	.line-group {
		display: flex;
		height: 100%;
	}
</style>
