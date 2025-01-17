<script lang="ts">
	import CurrentSeries from './CurrentSeries.svelte';
	import EmptySeries from './EmptySeries.svelte';
	import ErrorSeries from './ErrorSeries.svelte';
	import SeriesDividerLine from './SeriesDividerLine.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import CommitList from '$components/CommitList.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import SeriesHeader from '$components/SeriesHeader.svelte';
	import { isPatchSeries, type BranchStack } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { CommitDropData } from '$lib/dragging/draggables';
	import {
		StackingReorderDropzoneManagerFactory,
		buildNewStackOrder
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { getContext } from '@gitbutler/shared/context';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		stack: BranchStack;
		lastPush: Date | undefined;
	}

	const { stack: branch, lastPush }: Props = $props();

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

	function accepts(data: unknown) {
		if (!(data instanceof CommitDropData)) return false;
		if (data.branchId !== branch.id) return false;

		return true;
	}

	function onDrop(data: CommitDropData, allSeries: PatchSeries[], currentSeries: PatchSeries) {
		if (!(data instanceof CommitDropData)) return;

		const stackOrder = buildNewStackOrder(allSeries, currentSeries, data.commit.id, 'top');

		if (stackOrder) {
			branchController.reorderStackCommit(data.branchId, stackOrder);
		}
	}
</script>

{#each nonArchivedSeries as currentSeries, idx ('name' in currentSeries ? currentSeries.name : undefined)}
	{@const isTopBranch = idx === 0}
	{@const isBottomBranch = idx === nonArchivedSeries.length - 1}
	{#if !isTopBranch}
		<SeriesDividerLine
			topPatchStatus={isPatchSeries(currentSeries) ? currentSeries?.patches?.[0]?.status : 'error'}
		/>
	{/if}

	{#if !isError(currentSeries)}
		<CurrentSeries {currentSeries}>
			<SeriesHeader branch={currentSeries} {isTopBranch} {lastPush} />

			{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
				<div>
					<Dropzone
						{accepts}
						ondrop={(data) => onDrop(data, nonArchivedValidSeries, currentSeries)}
					>
						{#snippet overlay({ hovered, activated })}
							<CardOverlay {hovered} {activated} label="Move here" />
						{/snippet}
						<EmptySeries {isBottomBranch} />
					</Dropzone>
				</div>
			{/if}

			{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
				<CommitList
					{currentSeries}
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
