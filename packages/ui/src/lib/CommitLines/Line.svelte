<script lang="ts">
	import BaseNode from './BaseNode.svelte';
	import Cell from './Cell.svelte';
	import CommitNode from './CommitNode.svelte';
	import { pxToRem } from '../utils/pxToRem';
	import type { LineData } from './types';

	console.log('hello world!! :D');
	interface Props {
		line: LineData;
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
		background-color: green;
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
