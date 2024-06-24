<script lang="ts">
	import Line from '$lib/commitLines/Line.svelte';
	import type { LineGroup } from '$lib/commitLines/types';

	interface Props {
		lineGroup: LineGroup;
		topHeightPx?: number;
	}

	const { lineGroup, topHeightPx }: Props = $props();
</script>

<div class="line-group">
	{#each lineGroup.lines as line}
		<Line {line} {topHeightPx} />
	{/each}
</div>

<style lang="postcss">
	.line-group {
		display: flex;
		height: 100%;
	}
</style>
