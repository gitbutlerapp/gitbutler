<script lang="ts">
	import type { CellData } from '$lib/commitLinesStacking/types';

	interface Props {
		cell: CellData;
		isBottom?: boolean;
	}

	const { cell, isBottom = false }: Props = $props();
</script>

<div
	class="commit-line straight stacking-feature"
	class:none={cell.color === 'none'}
	class:remote={cell.color === 'remote'}
	class:local={cell.color === 'local'}
	class:local-and-remote={cell.color === 'localAndRemote'}
	class:shadow={cell.color === 'shadow'}
	class:integrated={cell.color === 'integrated'}
	class:dashed={cell.style === 'dashed' || isBottom}
></div>

<style lang="postcss">
	.straight {
		border-right: 2px var(--border-style) var(--border-color);
	}
</style>
