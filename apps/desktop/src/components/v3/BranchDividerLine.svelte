<script lang="ts">
	import { getColorFromBranchType } from '$components/v3/lib';
	import type { CommitStateType } from '$lib/branches/v3';

	interface Props {
		topPatchStatus: CommitStateType | 'Error';
	}

	const { topPatchStatus }: Props = $props();

	const branchType = $derived<CommitStateType | 'Error'>(topPatchStatus);
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
