<script lang="ts">
	import StackSeriesDividerLine from './StackSeriesDividerLine.svelte';
	import StackingSeriesHeader from '$lib/branch/StackingSeriesHeader.svelte';
	import StackingCommitList from '$lib/commit/StackingCommitList.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import { getLocalAndRemoteCommits, getLocalCommits } from '$lib/vbranches/contexts';
	import { getContext } from '@gitbutler/shared/context';
	import type { VirtualBranch } from '$lib/vbranches/types';

	interface Props {
		branch: VirtualBranch;
	}

	const { branch }: Props = $props();

	const localCommits = getLocalCommits();
	const localAndRemoteCommits = getLocalAndRemoteCommits();

	const localCommitsConflicted = $derived($localCommits.some((commit) => commit.conflicted));
	const localAndRemoteCommitsConflicted = $derived(
		$localAndRemoteCommits.some((commit) => commit.conflicted)
	);

	const reorderDropzoneManagerFactory = getContext(ReorderDropzoneManagerFactory);
	const reorderDropzoneManager = $derived(
		reorderDropzoneManagerFactory.build(branch, [...branch.localCommits, ...branch.remoteCommits])
	);
</script>

<!-- TODO: Add connecting line on background between NewStackCard above and branches below -->
{#each branch.series as currentSeries, idx (currentSeries.name)}
	{#if idx !== 0}
		<StackSeriesDividerLine {currentSeries} />
	{/if}
	<div class="branch-group">
		<StackingSeriesHeader {currentSeries} />
		{#if currentSeries.upstreamPatches.length > 0 || currentSeries.patches.length > 0}
			<StackingCommitList
				remoteOnlyPatches={currentSeries.upstreamPatches}
				patches={currentSeries.patches}
				isUnapplied={false}
				{reorderDropzoneManager}
				{localCommitsConflicted}
				{localAndRemoteCommitsConflicted}
			/>
		{/if}
	</div>
{/each}

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);

		&:last-child {
			margin-bottom: 12px;
		}
	}
</style>
