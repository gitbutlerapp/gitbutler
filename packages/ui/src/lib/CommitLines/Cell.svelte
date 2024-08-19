<script lang="ts">
	import Fork from '$lib/commitLines/Cell/Fork.svelte';
	import Straight from '$lib/commitLines/Cell/Straight.svelte';
	import type { CellData } from '$lib/commitLines/types';

	interface Props {
		cell: CellData;
		isBottom?: boolean;
	}

	const { cell, isBottom = false }: Props = $props();
</script>

{#if cell.type === 'fork'}
	<Fork color={cell.color} style={cell.style} {isBottom} />
{:else}
	<Straight color={cell.color} style={cell.style} />
{/if}
