<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import CommitList from '$components/CommitList.svelte';
	import CurrentSeries from '$components/CurrentSeries.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import EmptySeries from '$components/EmptySeries.svelte';
	import ErrorSeries from '$components/ErrorSeries.svelte';
	import SeriesDividerLine from '$components/SeriesDividerLine.svelte';
	import SeriesHeader from '$components/SeriesHeader.svelte';
	import { isPatchSeries, type BranchStack } from '$lib/branches/branch';
	import {
		ReorderCommitDzHandler,
		StackingReorderDropzoneManagerFactory
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		projectId: string;
		stack: BranchStack;
	}

	const { projectId, stack }: Props = $props();

	const stackService = getContext(StackService);

	// Must contain the errored series in order to render them in the list in the correct spot
	const nonArchivedSeries = $derived(
		stack.series.filter((s) => {
			if (isError(s)) return s;
			return !s.archived;
		})
	);

	// All non-errored non-archived series for consumption elsewhere
	const nonArchivedValidSeries = $derived(stack.validSeries.filter((s) => !s.archived));

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(stack)
	);
</script>

{#each nonArchivedSeries as currentSeries, idx ('name' in currentSeries ? currentSeries.name : undefined)}
	{@const isTopBranch = idx === 0}
	{@const isBottomBranch = idx === nonArchivedSeries.length - 1}
	{#if !isTopBranch}
		<SeriesDividerLine
			topPatchStatus={isPatchSeries(currentSeries) ? currentSeries?.patches?.[0]?.status : 'Error'}
		/>
	{/if}

	{#if !isError(currentSeries)}
		<CurrentSeries {currentSeries}>
			<SeriesHeader {projectId} branch={currentSeries} {isTopBranch} />

			{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
				{@const dzHandler = new ReorderCommitDzHandler(
					projectId,
					stack.id,
					stackService,
					currentSeries,
					nonArchivedValidSeries,
					'top'
				)}
				<div>
					<Dropzone handlers={[dzHandler]}>
						{#snippet overlay({ hovered, activated })}
							<CardOverlay {hovered} {activated} label="Move here" />
						{/snippet}
						<EmptySeries {isBottomBranch} />
					</Dropzone>
				</div>
			{/if}

			{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
				<CommitList
					{projectId}
					stackId={stack.id}
					{currentSeries}
					isUnapplied={false}
					isBottom={idx === stack.series.length - 1}
					{stackingReorderDropzoneManager}
				/>
			{/if}
		</CurrentSeries>
	{:else}
		<ErrorSeries error={currentSeries} />
	{/if}
{/each}
