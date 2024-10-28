<script lang="ts">
	import StackCurrentSeries from './StackCurrentSeries.svelte';
	import StackSeriesDividerLine from './StackSeriesDividerLine.svelte';
	import StackingSeriesHeader from '$lib/branch/StackingSeriesHeader.svelte';
	import StackingCommitList from '$lib/commit/StackingCommitList.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import type { StackingReorderDropzoneManager } from '$lib/dragging/stackingReorderDropzoneManager';
	import type { VirtualBranch } from '$lib/vbranches/types';

	interface Props {
		branch: VirtualBranch;
		stackingReorderDropzoneManager: StackingReorderDropzoneManager;
	}

	const { branch, stackingReorderDropzoneManager }: Props = $props();

	const hasConflicts = $derived(
		branch.series.flatMap((s) => s.patches).some((patch) => patch.conflicted)
	);

	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));
</script>

{#each nonArchivedSeries as currentSeries, idx (currentSeries.name)}
	{@const isTopSeries = idx === 0}
	{#if !isTopSeries}
		<StackSeriesDividerLine {currentSeries} />
	{/if}
	<StackCurrentSeries {currentSeries}>
		<StackingSeriesHeader {currentSeries} {isTopSeries} />

		{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
			<div class="branch-emptystate">
				<EmptyStatePlaceholder bottomMargin={10}>
					{#snippet title()}
						This is an empty branch
					{/snippet}
					{#snippet caption()}
						Create or drag and drop commits here
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{/if}

		{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
			<StackingCommitList
				remoteOnlyPatches={currentSeries.upstreamPatches}
				patches={currentSeries.patches}
				seriesName={currentSeries.name}
				isUnapplied={false}
				isBottom={idx === branch.series.length - 1}
				{stackingReorderDropzoneManager}
				{hasConflicts}
			/>
		{/if}
	</StackCurrentSeries>
{/each}
