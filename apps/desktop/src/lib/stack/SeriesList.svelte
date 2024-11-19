<script lang="ts">
	import CurrentSeries from './CurrentSeries.svelte';
	import EmptySeries from './EmptySeries.svelte';
	import ErrorSeries from './ErrorSeries.svelte';
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
	import { isPatchSeries, PatchSeries, type VirtualBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		branch: VirtualBranch;
		lastPush: Date | undefined;
	}

	const { branch, lastPush }: Props = $props();

	const branchController = getContext(BranchController);

	// Must contain the errored series in order to render them in the list in the correct spot
	const nonArchivedSeries = $derived(
		branch.series.filter((s) => {
			if (isError(s)) return s;
			return !s.archived;
		})
	);

	// All non-errored non-archived series for consumption elsewhere
	const nonArchivedValidSeries = $derived(branch.validSeries.filter((s) => !s.archived));

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

{#each nonArchivedSeries as currentSeries, idx (currentSeries)}
	{@const isTopSeries = idx === 0}
	{@const isBottomSeries = idx === branch.series.length - 1}
	{#if !isTopSeries}
		<SeriesDividerLine
			topPatchStatus={isPatchSeries(currentSeries) ? currentSeries?.patches?.[0]?.status : 'error'}
		/>
	{/if}

	{#if !isError(currentSeries)}
		<CurrentSeries {currentSeries}>
			<SeriesHeader {currentSeries} {isTopSeries} {lastPush} />

			{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
				<div>
					<Dropzone
						{accepts}
						ondrop={(data) => onDrop(data, nonArchivedValidSeries, currentSeries)}
					>
						{#snippet overlay({ hovered, activated })}
							<CardOverlay {hovered} {activated} label="Move here" />
						{/snippet}
						<EmptySeries isBottom={isBottomSeries} />
					</Dropzone>
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
				/>
			{/if}
		</CurrentSeries>
	{:else}
		<ErrorSeries error={currentSeries} />
	{/if}
{/each}
