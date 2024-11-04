<script lang="ts">
	import Cell from '$lib/commitLines/Cell.svelte';
	import CommitNode from '$lib/commitLines/CommitNode.svelte';
	import type { LineData } from '$lib/commitLines/types';

	interface Props {
		line: LineData;
		isBottom?: boolean;
	}

	const { line, isBottom = false }: Props = $props();
</script>

<div class="line">
	<div class="line-top">
		<Cell cell={line.top} />
	</div>
	{#if line.commitNode}
		<CommitNode commitNode={line.commitNode} type={line.commitNode.type ?? 'local'} />
	{/if}
	<div class="line-bottom">
		<Cell cell={line.bottom} {isBottom} />
	</div>
</div>

<style lang="postcss">
	.line {
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		align-items: flex-end;
		width: 25px;
		margin-right: 8px;
	}

	.line-top {
		height: 14px;
		width: 100%;
	}

	.line-bottom {
		flex-grow: 1;
		width: 100%;
	}
</style>
