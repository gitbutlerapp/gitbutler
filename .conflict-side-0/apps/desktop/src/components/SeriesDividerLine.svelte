<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { CommitStatus } from '$lib/commits/commit';

	type SeriesStatus = CommitStatus | 'Error';

	interface Props {
		topPatchStatus?: SeriesStatus;
	}

	const { topPatchStatus }: Props = $props();

	const branchType = $derived<SeriesStatus>(topPatchStatus ?? 'LocalOnly');
	const lineColor = $derived(getColorFromBranchType(branchType));
</script>

<div class="commit-line" style:--commit-color={lineColor}></div>

<style>
	.commit-line {
		height: 10px;
		margin: 0 21px;
		background-color: var(--commit-color);
	}
</style>
