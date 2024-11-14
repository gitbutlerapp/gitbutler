<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { CommitStatus, PatchSeries } from '$lib/vbranches/types';

	interface Props {
		currentSeries: PatchSeries;
	}

	const { currentSeries }: Props = $props();

	const topPatch = $derived(currentSeries?.patches[0]);
	const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	const lineColor = $derived(getColorFromBranchType(branchType));
</script>

<div class="commit-line" style:--commit-color={lineColor}></div>

<style>
	.commit-line {
		width: 2px;
		height: 10px;
		margin: 0 21px;
		background-color: var(--commit-color);
	}
</style>
