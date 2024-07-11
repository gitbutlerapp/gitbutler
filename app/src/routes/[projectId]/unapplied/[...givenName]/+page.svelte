<script lang="ts">
	import { BranchService } from '$lib/branches/service';
	import { getContext } from '$lib/utils/context';
	import { onMount } from 'svelte';
	import type { CombinedBranch } from '$lib/branches/types';
	import { page } from '$app/stores';

	// Maintain combined branches as a state variable
	let combinedBranches: CombinedBranch[] = $state([]);
	onMount(() => {
		branchService.branches$.subscribe((newCombinedBranches) => {
			combinedBranches = newCombinedBranches;
		});
	});

	const branchService = getContext(BranchService);

	const givenName = $derived($page.params.givenName);

	let combinedBranch: CombinedBranch | undefined = $derived(
		combinedBranches.find((combinedBranch) => combinedBranch.givenName === givenName)
	);

	// There should only ever be at most 1 local branch
	const localBranch = $derived(combinedBranch?.branches.find((branch) => !branch.isRemote));
</script>

{#if !combinedBranch}
	<p>Loading...</p>
{:else}
	<p>{combinedBranch.givenName}</p>
	<p>{combinedBranch.pullRequests.length}</p>
	<p>-</p>
	<p>{combinedBranch.branches.length}</p>
	<p>{localBranch?.name}</p>
{/if}
