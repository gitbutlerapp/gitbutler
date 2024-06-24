<script lang="ts">
	import BaseNode from '$lib/commitLines/BaseNode.svelte';
	import Cell from '$lib/commitLines/Cell.svelte';
	import CommitNode from '$lib/commitLines/CommitNode.svelte';
	import type { Line } from '$lib/commitLines/types';

	interface Props {
		line: Line;
	}

	const { line }: Props = $props();
</script>

<div class="line">
	<div class="line-top" class:taller-top={line.tallerTop} class:has-branch-node={line.baseNode}>
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
	}

	.line-top {
		height: 24px;
		width: 100%;
		&.taller-top {
			height: 58px;
		}

		&.has-branch-node {
			height: 24px;
		}
	}

	.line-bottom {
		flex-grow: 1;
		width: 100%;
	}
</style>
