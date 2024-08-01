<script lang="ts">
	import { BranchListingService } from '$lib/branches/branchListing';
	import BranchPreview from '$lib/components/BranchPreview.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { RemoteBranchService } from '$lib/stores/remoteBranches';
	import { getContext } from '$lib/utils/context';
	import { groupBy } from '$lib/utils/groupBy';
	import type { Branch } from '$lib/vbranches/types';
	import { page } from '$app/stores';

	const branchListingService = getContext(BranchListingService);
	const branchListings = branchListingService.branchListings;

	const remoteBranchService = getContext(RemoteBranchService);
	const branches = remoteBranchService.branches;
	const branchesByGivenName = $derived(groupBy($branches, (branch) => branch.givenName));

	const branchListing = $derived($branchListings.find((bl) => bl.name === $page.params.name));

	const gitHostListingService = getGitHostListingService();
	const prs = $derived($gitHostListingService?.prs);
	const pr = $derived($prs?.find((pr) => pr.sourceBranch === branchListing?.name));

	let localBranch = $state<Branch>();
	let remoteBranches = $state<Branch[]>([]);

	$effect(() => {
		if (branchListing) {
			const branchesWithGivenName = branchesByGivenName[branchListing.name];

			localBranch = branchesWithGivenName.find((branch) => !branch.isRemote);

			remoteBranches = branchesWithGivenName.filter((branch) => branch.isRemote);
		}
	});
</script>

{#if $branchListings.length === 0}
	<FullviewLoading />
{:else if branchListing}
	{#if remoteBranches.length === 0 && localBranch}
		<BranchPreview {localBranch} {pr} />
	{:else}
		{#each remoteBranches as remoteBranch}
			<BranchPreview {localBranch} {remoteBranch} {pr} />
		{/each}
	{/if}
{:else}
	<p>Branch doesn't seem to exist</p>
{/if}
