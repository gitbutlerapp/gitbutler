<script lang="ts">
	import StackingBranchHeader from '$lib/branch/StackingBranchHeader.svelte';
	import StackingCommitList from '$lib/commit/StackingCommitList.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import { getContext } from '$lib/utils/context';
	import {
		getLocalAndRemoteCommits,
		getLocalCommits,
		getRemoteCommits
	} from '$lib/vbranches/contexts';
	import type { VirtualBranch } from '$lib/vbranches/types';
	// import type { Series } from './types';

	interface Props {
		// series: Series[];
		branch: VirtualBranch;
	}

	const { branch }: Props = $props();

	const localCommits = getLocalCommits();
	const localAndRemoteCommits = getLocalAndRemoteCommits();
	const remoteCommits = getRemoteCommits();

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
{#each branch.series as currentSeries}
	<div class="branch-group">
		<StackingBranchHeader
			commits={currentSeries.patches}
			name={currentSeries.branchName}
			upstreamName={currentSeries.name}
		/>
		<StackingCommitList
			localCommits={currentSeries.localCommits}
			localAndRemoteCommits={currentSeries.remoteCommits}
			integratedCommits={currentSeries.integratedCommits}
			remoteCommits={$remoteCommits}
			isUnapplied={false}
			{reorderDropzoneManager}
			{localCommitsConflicted}
			{localAndRemoteCommitsConflicted}
		/>
	</div>
{/each}

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		overflow: hidden;
	}
</style>
