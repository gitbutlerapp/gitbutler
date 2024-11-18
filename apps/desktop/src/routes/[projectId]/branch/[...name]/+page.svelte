<script lang="ts">
	import { GitBranchService } from '$lib/branches/gitBranch';
	import BranchPreview from '$lib/components/BranchPreview.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { Branch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { page } from '$app/stores';

	const gitBranchService = getContext(GitBranchService);

	const forgeListingService = getForgeListingService();
	const name = $derived($page.params.name);
	const prs = $derived($forgeListingService?.prs);
	const pr = $derived($prs?.find((pr) => pr.sourceBranch === name));

	let localBranch = $state<Branch>();
	let remoteBranches = $state<Branch[]>([]);
	let loading = $state(false);

	$effect(() => {
		if (!name) return;
		findBranches(name);
	});

	async function findBranches(name: string) {
		loading = true;
		try {
			const branches = await gitBranchService.findBranches(name);
			localBranch = branches.find((branch) => !branch.isRemote);
			remoteBranches = branches.filter((branch) => branch.isRemote);
		} finally {
			loading = false;
		}
	}
</script>

{#if loading}
	<FullviewLoading />
{:else}
	{#if localBranch && remoteBranches.length === 0}
		<BranchPreview {localBranch} {pr} />
	{:else}
		{#each remoteBranches as remoteBranch}
			<BranchPreview {remoteBranch} {pr} />
		{/each}
	{/if}
	{#if !localBranch && remoteBranches.length === 0}
		<p>Branch doesn't seem to exist</p>
	{/if}
{/if}
