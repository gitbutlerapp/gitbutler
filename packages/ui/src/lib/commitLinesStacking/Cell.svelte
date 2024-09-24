<script lang="ts">
	import type { CellData } from '$lib/commitLinesStacking/types';

	interface Props {
		cell: CellData;
		isBottom?: boolean;
	}

	const { cell, isBottom = false }: Props = $props();
</script>

<div
	class="commit-line"
	class:local={cell.type === 'Local'}
	class:remote={cell.type === 'Remote'}
	class:local-and-remote={cell.type === 'LocalShadow'}
	class:upstream={cell.type === 'Upstream'}
	class:integrated={cell.type === 'Integrated'}
	class:dashed={cell.style === 'dashed' || isBottom}
></div>

<style lang="postcss">
	.commit-line {
		border-right: 2px var(--border-style) var(--border-color);
	}
</style>
