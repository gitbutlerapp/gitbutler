<script lang="ts">
	import CurrentSeries from './CurrentSeries.svelte';
	import SeriesDividerLine from './SeriesDividerLine.svelte';
	import SeriesHeader from '$lib/branch/SeriesHeader.svelte';
	import CommitList from '$lib/commit/CommitList.svelte';
	import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
	import { getContext } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import type { VirtualBranch } from '$lib/vbranches/types';

	interface Props {
		branch: VirtualBranch;
	}

	const { branch }: Props = $props();

	const hasConflicts = $derived(
		branch.series.flatMap((s) => s.patches).some((patch) => patch.conflicted)
	);

	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(branch)
	);
</script>

{#each nonArchivedSeries as currentSeries, idx (currentSeries.name)}
	{@const isTopSeries = idx === 0}
	{#if !isTopSeries}
		<SeriesDividerLine {currentSeries} />
	{/if}
	<CurrentSeries {currentSeries}>
		<SeriesHeader {currentSeries} {isTopSeries} />

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
			<CommitList
				remoteOnlyPatches={currentSeries.upstreamPatches}
				patches={currentSeries.patches}
				seriesName={currentSeries.name}
				isUnapplied={false}
				isBottom={idx === branch.series.length - 1}
				{stackingReorderDropzoneManager}
				{hasConflicts}
			/>
		{/if}
	</CurrentSeries>
{/each}
