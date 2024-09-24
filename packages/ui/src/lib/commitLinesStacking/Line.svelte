<script lang="ts">
	import Cell from '$lib/commitLinesStacking/Cell.svelte';
	import CommitNode from '$lib/commitLinesStacking/CommitNode.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { LineData } from '$lib/commitLinesStacking/types';

	interface Props {
		line: LineData;
		topHeightPx?: number;
	}

	const { line, topHeightPx = 12 }: Props = $props();
	console.log('LINE', line);
</script>

<div class="line">
	<div class="line-top" style:--top-height={pxToRem(topHeightPx)}>
		<Cell cell={line.top} />
	</div>
	{#if line.commitNode}
		<CommitNode commitNode={line.commitNode} color={line.commitNode.type ?? 'Local'} />
	{/if}
	<div class="line-bottom">
		<Cell cell={line.bottom} isBottom />
	</div>
</div>

<style lang="postcss">
	.line {
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		align-items: flex-end;
		width: 24px;
		margin-right: -2px;
	}

	.line-top {
		height: var(--top-height);
		width: 100%;
	}

	.line-bottom {
		flex-grow: 1;
		width: 100%;
	}
</style>
