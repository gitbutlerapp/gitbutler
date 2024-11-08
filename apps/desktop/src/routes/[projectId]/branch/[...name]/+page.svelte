<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { BranchListingService } from '$lib/branches/branchListing';
	import BranchPreview from '$lib/components/BranchPreview.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { RemoteBranchService } from '$lib/stores/remoteBranches';
	import { groupBy } from '$lib/utils/groupBy';
	import { error } from '$lib/utils/toasts';
	import { PartialGitBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const project = getContext(Project);
	const branchListingService = getContext(BranchListingService);
	const branchListings = branchListingService.branchListings;

	const remoteBranchService = getContext(RemoteBranchService);
	const branches = remoteBranchService.branches;
	const branchesByGivenName = $derived(groupBy($branches, (branch) => branch.givenName));

	const branchListing = $derived($branchListings.find((bl) => bl.name === $page.params.name));

	const forgeListingService = getForgeListingService();
	const prs = $derived($forgeListingService?.prs);
	const pr = $derived($prs?.find((pr) => pr.sourceBranch === branchListing?.name));

	let localBranch = $state<PartialGitBranch>();
	let remoteBranches = $state<PartialGitBranch[]>([]);

	$effect(() => {
		if (branchListing) {
			if (branchListing.virtualBranch?.inWorkspace) {
				goto(`/${project.id}/board`);
				return;
			}

			const branchesWithGivenName: PartialGitBranch[] | undefined =
				branchesByGivenName[branchListing.name];

			if (branchesWithGivenName) {
				localBranch = branchesWithGivenName.find((branch) => !branch.isRemote);

				remoteBranches = branchesWithGivenName.filter((branch) => branch.isRemote);
			} else {
				error('Failed to find branch');
				goto(`/${project.id}/board`);
			}
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
