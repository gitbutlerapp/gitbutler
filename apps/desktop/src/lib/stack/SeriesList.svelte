<script lang="ts">
	import CurrentSeries from './CurrentSeries.svelte';
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
	import { Branch, type BranchStack } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';

	interface Props {
		stack: BranchStack;
	}

	const { stack }: Props = $props();

	const branchController = getContext(BranchController);
	const hasConflicts = $derived(
		stack.branches.flatMap((s) => s.patches).some((patch) => patch.conflicted)
	);

	const nonArchivedSeries = $derived(stack.branches.filter((s) => !s.archived));

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(stack)
	);

	function accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== stack.id) return false;

		return true;
	}

	function onDrop(data: DraggableCommit, allSeries: Branch[], branch: Branch) {
		if (!(data instanceof DraggableCommit)) return;

		const stackOrder = buildNewStackOrder(allSeries, branch, data.commit.id, 'top');

		if (stackOrder) {
			branchController.reorderStackCommit(data.branchId, stackOrder);
		}
	}
</script>

{#each nonArchivedSeries as branch, idx}
	{@const isTopSeries = idx === 0}
	{#if !isTopSeries}
		<SeriesDividerLine currentSeries={branch} />
	{/if}
	<CurrentSeries currentSeries={branch}>
		<SeriesHeader {branch} {isTopSeries} />

		{#if branch.upstreamPatches.length === 0 && branch.patches.length === 0}
			<div class="branch-emptystate">
				<Dropzone {accepts} ondrop={(data) => onDrop(data, nonArchivedSeries, branch)}>
					{#snippet overlay({ hovered, activated })}
						<CardOverlay {hovered} {activated} label="Move here" />
					{/snippet}
					<EmptyStatePlaceholder bottomMargin={10}>
						{#snippet title()}
							This is an empty branch
						{/snippet}
						{#snippet caption()}
							Create or drag and drop commits here
						{/snippet}
					</EmptyStatePlaceholder>
				</Dropzone>
			</div>
		{/if}

		{#if branch.upstreamPatches.length > 0 || branch.patches.length > 0}
			<CommitList
				remoteOnlyPatches={branch.upstreamPatches}
				patches={branch.patches}
				seriesName={branch.name}
				isUnapplied={false}
				isBottom={idx === stack.branches.length - 1}
				{stackingReorderDropzoneManager}
				{hasConflicts}
			/>
		{/if}
	</CurrentSeries>
{/each}
