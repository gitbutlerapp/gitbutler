<script lang="ts">
	import StackCurrentSeries from './StackCurrentSeries.svelte';
	import StackSeriesDividerLine from './StackSeriesDividerLine.svelte';
	import StackingSeriesHeader from '$lib/branch/StackingSeriesHeader.svelte';
	import StackingCommitList from '$lib/commit/StackingCommitList.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import { getContext } from '@gitbutler/shared/context';
	import type { VirtualBranch } from '$lib/vbranches/types';

	interface Props {
		branch: VirtualBranch;
	}

	const { branch }: Props = $props();

	const hasConflicts = $derived(
		branch.series.flatMap((s) => s.patches).some((patch) => patch.conflicted)
	);

	const reorderDropzoneManagerFactory = getContext(ReorderDropzoneManagerFactory);
	const reorderDropzoneManager = $derived(
		reorderDropzoneManagerFactory.build(branch, [...branch.localCommits, ...branch.remoteCommits])
	);
</script>

{#each branch.series as currentSeries, idx (currentSeries.name)}
	{@const isTopSeries = idx === 0}
	{#if !isTopSeries}
		<StackSeriesDividerLine {currentSeries} />
	{/if}
	<StackCurrentSeries {currentSeries}>
		<StackingSeriesHeader {currentSeries} {isTopSeries} />
		{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
			<StackingCommitList
				remoteOnlyPatches={currentSeries.upstreamPatches}
				patches={currentSeries.patches}
				seriesName={currentSeries.name}
				isUnapplied={false}
				isBottom={idx === branch.series.length - 1}
				{reorderDropzoneManager}
				{hasConflicts}
			/>
		{/if}
	</StackCurrentSeries>
{/each}
