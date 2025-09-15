<script lang="ts">
	import Cell from '$components/commitLines/Cell.svelte';
	import CommitNode from '$components/commitLines/CommitNode.svelte';
	import { getColorFromBranchType } from '$lib/utils/getColorFromBranchType';
	import type { CellType, CommitNodeData } from '$components/commitLines/types';

	interface Props {
		line: CommitNodeData;
		isBottom?: boolean;
	}

	const { line, isBottom = false }: Props = $props();

	const lineType = $derived<CellType>(line.type ?? 'LocalOnly');
</script>

<div class="line" style:--commit-color={getColorFromBranchType(lineType)}>
	<div class="line-top">
		<Cell />
	</div>
	{#if line.commit}
		<CommitNode
			commitNode={{
				commit: line.commit,
				type: line.type
			}}
		/>
	{/if}
	<div class="line-bottom">
		<Cell {isBottom} />
	</div>
</div>

<style lang="postcss">
	.line {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		width: 25px;
		height: 100%;
		margin-right: 8px;
		gap: 0.2rem;
	}

	.line-top {
		width: 100%;
		height: 14px;
	}

	.line-bottom {
		flex-grow: 1;
		width: 100%;
	}
</style>
