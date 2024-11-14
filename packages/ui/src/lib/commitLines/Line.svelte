<script lang="ts">
	import Cell from '$lib/commitLines/Cell.svelte';
	import CommitNode from '$lib/commitLines/CommitNode.svelte';
	import { getColorFromBranchType } from '$lib/utils/getColorFromBranchType';
	import type { CellType, CommitNodeData } from '$lib/commitLines/types';

	interface Props {
		line: CommitNodeData;
		isBottom?: boolean;
		typeOverride?: CellType;
	}

	const { line, typeOverride, isBottom = false }: Props = $props();

	const lineType = $derived<CellType>(line.type ?? line.type ?? 'local');
</script>

<div class="line" style:--commit-color={getColorFromBranchType(typeOverride ?? lineType)}>
	<div class="line-top">
		<Cell />
	</div>
	{#if line.commit}
		<CommitNode
			commitNode={{
				commit: line.commit,
				type: typeOverride ?? line.type
			}}
		/>
	{/if}
	<div class="line-bottom">
		<Cell {isBottom} />
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
