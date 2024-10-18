<script lang="ts">
	import StackCurrentSeries from './StackCurrentSeries.svelte';
	import StackSeriesDividerLine from './StackSeriesDividerLine.svelte';
	import StackingSeriesHeader from '$lib/branch/StackingSeriesHeader.svelte';
	import StackingCommitList from '$lib/commit/StackingCommitList.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
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

	const commits = $derived.by(() => {
		console.log('DERIVED.COMMITS.BRANCH_SERIES', branch.series);
		return branch.series.flatMap((series) => {
			let patches = [`top|${series.name}`];
			patches.push(...series.patches.map((patch) => patch.id));
			return patches;
		});
	});

	const reorderDropzoneManagerFactory = getContext(ReorderDropzoneManagerFactory);
	const reorderDropzoneManager = $derived(reorderDropzoneManagerFactory.build(commits));
</script>

{#each branch.series as currentSeries, idx (currentSeries.name)}
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
				{reorderDropzoneManager}
				{hasConflicts}
			/>
		{/if}
	</StackCurrentSeries>
{/each}
