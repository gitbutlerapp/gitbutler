<script lang="ts">
	import BranchPreview from '$components/BranchPreview.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import PageLoadFailed from '$components/PageLoadFailed.svelte';
	import { BranchData } from '$lib/branches/branch';
	import { GitBranchService } from '$lib/branches/gitBranch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId!);

	const [gitBranchService, forge] = inject(GitBranchService, DefaultForgeFactory);

	const name = $derived(page.params.name!);
	const forgeListingService = $derived(forge.current.listService);
	const prResult = $derived(forgeListingService?.getByBranch(projectId, name));
	const pr = $derived(prResult?.current.data);

	let localBranch = $state<BranchData>();
	let remoteBranches = $state<BranchData[]>([]);
	let loading = $state(false);
	let error = $state<unknown>();

	$effect(() => {
		if (!name) return;
		findBranches(name);
	});

	async function findBranches(name: string) {
		loading = true;
		error = undefined;
		try {
			const branches = await gitBranchService.findBranches(name);
			localBranch = branches.find((branch) => !branch.isRemote);
			remoteBranches = branches.filter((branch) => branch.isRemote);
		} catch (err) {
			console.error(err);
			error = err;
		} finally {
			loading = false;
		}
	}
</script>

{#if error}
	<PageLoadFailed {error} />
{:else if loading}
	<FullviewLoading />
{:else}
	{#if localBranch && remoteBranches.length === 0}
		<BranchPreview {projectId} {localBranch} {pr} />
	{:else}
		{#each remoteBranches as remoteBranch}
			<BranchPreview {projectId} {remoteBranch} {pr} />
		{/each}
	{/if}
	{#if !localBranch && remoteBranches.length === 0}
		<p>Branch doesn't seem to exist</p>
	{/if}
{/if}
