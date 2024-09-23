<script lang="ts">
	import Line from '$lib/commitLinesStacking/Line.svelte';
	import type { LineGroupData } from '$lib/commitLinesStacking/types';

	interface Props {
		lineGroup: LineGroupData;
		topHeightPx?: number;
		stackingFeature?: boolean;
	}

	const { lineGroup, topHeightPx, stackingFeature = false }: Props = $props();
	$inspect('lineGroup', lineGroup);
</script>

<div class="line-group" class:stacking-feature={stackingFeature}>
	<!-- {#each lineGroup.lines as line} -->
	<!-- 	<Line {line} {topHeightPx} {stackingFeature} /> -->
	<!-- {/each} -->
	<Line line={lineGroup.lines[1]} {topHeightPx} {stackingFeature} />
</div>

<style lang="postcss">
	.line-group {
		display: flex;
		height: 100%;

		&.stacking-feature {
			width: 48px;
		}
	}
</style>
