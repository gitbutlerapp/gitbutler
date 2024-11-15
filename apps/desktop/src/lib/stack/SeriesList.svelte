<script lang="ts">
	import CurrentSeries from './CurrentSeries.svelte';
	import EmptySeries from './EmptySeries.svelte';
	import SeriesDividerLine from './SeriesDividerLine.svelte';
	import SeriesHeader from '$lib/branch/SeriesHeader.svelte';
	import CommitList from '$lib/commit/CommitList.svelte';
	import { DraggableCommit } from '$lib/dragging/draggables';
	import {
		StackingReorderDropzoneManagerFactory,
		buildNewStackOrder
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import CardOverlay from '$lib/dropzone/CardOverlay.svelte';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { PatchSeries, type VirtualBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		branch: VirtualBranch;
		lastPush: Date | undefined;
	}

	const { branch, lastPush }: Props = $props();

	const branchController = getContext(BranchController);

	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(branch)
	);

	function accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== branch.id) return false;

		return true;
	}

	function onDrop(data: DraggableCommit, allSeries: PatchSeries[], currentSeries: PatchSeries) {
		if (!(data instanceof DraggableCommit)) return;

		const stackOrder = buildNewStackOrder(allSeries, currentSeries, data.commit.id, 'top');

		if (stackOrder) {
			branchController.reorderStackCommit(data.branchId, stackOrder);
		}
	}
</script>

{#each nonArchivedSeries as currentSeries, idx (currentSeries.name)}
	{@const isTopSeries = idx === 0}
	{@const isBottomSeries = idx === branch.series.length - 1}
	{#if !isTopSeries}
		<SeriesDividerLine currentSeries={nonArchivedSeries[idx - 1] as PatchSeries} />
	{/if}

	<CurrentSeries {currentSeries}>
		<SeriesHeader {currentSeries} {isTopSeries} {lastPush} />

		{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
			<div class="branch-emptystate">
				<Dropzone {accepts} ondrop={(data) => onDrop(data, nonArchivedSeries, currentSeries)}>
					{#snippet overlay({ hovered, activated })}
						<CardOverlay {hovered} {activated} label="Move here" />
					{/snippet}
					<EmptySeries isBottom={isBottomSeries} />
				</Dropzone>
			</div>
		{/if}

		{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
			<CommitList
				remoteOnlyPatches={currentSeries.upstreamPatches.filter((p) => !p.relatedTo)}
				patches={currentSeries.patches}
				seriesName={currentSeries.name}
				isUnapplied={false}
				isBottom={idx === branch.series.length - 1}
				{stackingReorderDropzoneManager}
			/>
		{/if}
	</CurrentSeries>
{/each}
