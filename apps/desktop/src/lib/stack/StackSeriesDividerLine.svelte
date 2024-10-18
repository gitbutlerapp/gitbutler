<script lang="ts">
	import { getColorFromBranchType } from '$lib/branch/stackingUtils';
	import type { CommitStatus, PatchSeries } from '$lib/vbranches/types';

	interface Props {
		currentSeries: PatchSeries;
	}

	const { currentSeries }: Props = $props();

	const topPatch = $derived(currentSeries?.patches[0]);
	const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	const lineColor = $derived(getColorFromBranchType(branchType));
</script>

<div class="stack-line" style:--bg-color={lineColor}></div>

<style>
	.stack-line {
		width: 2px;
		height: 10px;
		margin: 0 21px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>
