<script lang="ts">
	import BranchPreview from '$components/BranchPreview.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import PageLoadFailed from '$components/PageLoadFailed.svelte';
	import { GitBranchService } from '$lib/branches/gitBranch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import type { BranchData } from '$lib/branches/branch';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId!);

	const stackId = $derived(page.params.stackId!);
	const stackService = getContext(StackService);
	const gitBranchService = getContext(GitBranchService);
	const forge = getContext(DefaultForgeFactory);
	const forgeListingService = $derived(forge.current.listService);
	const stackResult = $derived(stackService.allStackById(projectId, stackId));

	// TODO: It would be nice to work on the same data structures that the
	// workspace page uses.
	let localBranch = $state<BranchData>();
	let remoteBranches = $state<BranchData[]>([]);
	let error = $state<unknown>();

	const stack = $derived(stackResult.current.data);
	const name = $derived(stack ? getStackName(stack) : undefined);

	const prResult = $derived(name ? forgeListingService?.getByBranch(projectId, name) : undefined);
	const pr = $derived(prResult?.current.data);

	$effect(() => {
		if (!name) return;
		findBranches(name);
	});

	async function findBranches(name: string) {
		error = undefined;
		try {
			const branches = await gitBranchService.findBranches(name);
			localBranch = branches.find((branch) => !branch.isRemote);
			remoteBranches = branches.filter((branch) => branch.isRemote);
		} catch (err) {
			console.error(err);
			error = err;
		}
	}
</script>

{#if error}
	<PageLoadFailed {error} />
{:else if !localBranch && remoteBranches.length === 0}
	<FullviewLoading />
{:else if localBranch && remoteBranches.length === 0}
	<BranchPreview {projectId} {localBranch} {pr} />
{:else}
	{#each remoteBranches as remoteBranch}
		<BranchPreview {projectId} {remoteBranch} {pr} />
	{/each}
{/if}
