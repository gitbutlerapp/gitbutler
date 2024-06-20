<script lang="ts">
	import Cell from '$lib/commitLines/Cell.svelte';
	import CommitNode from '$lib/commitLines/CommitNode.svelte';
	import type { Line } from '$lib/commitLines/types';

	interface Props {
		line: Line;
	}

	const { line }: Props = $props();
</script>

<div class="line">
	<div class="line-top">
		<Cell cell={line.top} />
	</div>
	{#if line.node}
		<CommitNode commitNode={line.node} />
	{/if}
	<div class="line-bottom">
		<Cell cell={line.bottom} />
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
		flex-grow: 1;
		width: 100%;
	}

	.line-bottom {
		height: 42px;
		width: 100%;
	}
</style>
