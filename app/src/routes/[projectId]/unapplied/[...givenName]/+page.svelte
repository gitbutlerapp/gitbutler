<script lang="ts">
	import { BranchService } from '$lib/branches/service';
	import { getContext } from '$lib/utils/context';
	import BranchLane from '@gitbutler/ui/branch/BranchLane.svelte';
	import { onMount } from 'svelte';
	import type { GivenNameBranchGrouping } from '$lib/branches/types';
	import { page } from '$app/stores';

	// Maintain combined branches as a state variable
	let combinedBranches: GivenNameBranchGrouping[] = $state([]);
	onMount(() => {
		branchService.branches$.subscribe((newCombinedBranches) => {
			combinedBranches = newCombinedBranches;
		});
	});

	const branchService = getContext(BranchService);

	const givenName = $derived($page.params.givenName);

	let combinedBranch: GivenNameBranchGrouping | undefined = $derived(
		combinedBranches.find((combinedBranch) => combinedBranch.givenName === givenName)
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
		<BranchLane fileCardExpanded={!!selectedFile} branchCardCollapsed={false}>
			{#snippet branchCard()}
				<div>Hi</div>
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
