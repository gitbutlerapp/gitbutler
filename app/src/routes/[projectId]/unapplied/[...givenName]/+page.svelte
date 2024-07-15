<script lang="ts">
	import { getBranchServiceStore } from '$lib/branches/service';
	import BranchCard from '@gitbutler/ui/branch/BranchCard.svelte';
	import BranchLane from '@gitbutler/ui/branch/BranchLane.svelte';
	import type { GivenNameBranchGrouping } from '$lib/branches/types';
	import { page } from '$app/stores';

	const branchServiceStore = getBranchServiceStore();
	const combinedBranches = $derived($branchServiceStore?.branches);

	const givenName = $derived($page.params.givenName);

	let combinedBranch: GivenNameBranchGrouping | undefined = $derived(
		$combinedBranches?.find((combinedBranch) => combinedBranch.givenName === givenName)
	);

	// There should only ever be at most 1 local branch
	const _localBranch = $derived(combinedBranch?.branches.find((branch) => !branch.isRemote));
	const _nonLocalBranches = $derived(
		combinedBranch?.branches.filter((branch) => branch.isRemote) || []
	);

	const selectedFile = $state(undefined);
</script>

{#if !combinedBranch}
	<p>Loading...</p>
{:else}
	<!--
		<p>{combinedBranch.givenName}</p>
		<p>{combinedBranch.pullRequests.length}</p>
		<p>-</p>
		<p>{combinedBranch.branches.length}</p>
		<p>{localBranch?.name}</p>
	-->

	<div class="board">
		<BranchLane fileCardExpanded={!!selectedFile} isLaneCollapsed={false}>
			{#snippet branchCard()}
				<BranchCard isLaneCollapsed={false}>
					{#snippet branchHeader()}
						<p>I'm the header!</p>
					{/snippet}
					{#snippet branchFiles()}
						<p>I'm the branch files!</p>
					{/snippet}
					{#snippet branchFooter()}
						<p>I'm the branch footer!</p>
					{/snippet}
				</BranchCard>
			{/snippet}
			{#snippet fileCard()}
				<div>File card</div>
			{/snippet}
		</BranchLane>
	</div>
{/if}

<style lang="postcss">
	.board {
		display: flex;
		gap: 8px;
	}
</style>
