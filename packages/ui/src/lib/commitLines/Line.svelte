<script lang="ts">
	import BaseNode from '$lib/commitLines/BaseNode.svelte';
	import Cell from '$lib/commitLines/Cell.svelte';
	import CommitNode from '$lib/commitLines/CommitNode.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { Line } from '$lib/commitLines/types';

	interface Props {
		line: Line;
		topHeightPx?: number;
	}

	const { line, topHeightPx = 24 }: Props = $props();
</script>

<div class="line">
	<div
		class="line-top"
		style:--top-height={pxToRem(topHeightPx)}
		class:has-branch-node={line.baseNode}
	>
		<Cell cell={line.top} />
	</div>
	{#if line.commitNode}
		<CommitNode commitNode={line.commitNode} color={line.bottom.color} />
	{:else if line.baseNode}
		<BaseNode baseNode={line.baseNode} color={line.top.color} />
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
		align-items: flex-end;
		width: 24px;
		margin-right: -2px;
	}

	.line-top {
		height: var(--top-height);
		width: 100%;

		&.has-branch-node {
			height: 24px;
		}
	}

	.line-bottom {
		flex-grow: 1;
		width: 100%;
	}
</style>
