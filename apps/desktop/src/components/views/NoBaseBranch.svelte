<script lang="ts">
	import ProjectSetup from "$components/onboarding/ProjectSetup.svelte";
	import FullviewLoading from "$components/shared/FullviewLoading.svelte";
	import ProblemLoadingRepo from "$components/views/ProblemLoadingRepo.svelte";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { inject } from "@gitbutler/core/context";

	// `notice` says why the target needs choosing again, if it was set before.
	const { projectId, notice }: { projectId: string; notice?: string } = $props();
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const remoteBranchesQuery = $derived(baseBranchService.remoteBranches(projectId));
	const localBranchesQuery = $derived(baseBranchService.localTargetBranches(projectId));
</script>

{#if remoteBranchesQuery.result.isLoading || localBranchesQuery.result.isLoading}
	<FullviewLoading />
{:else if remoteBranchesQuery.result.isSuccess && localBranchesQuery.result.isSuccess}
	{@const remoteBranches = remoteBranchesQuery.response ?? []}
	{@const localBranches = localBranchesQuery.response ?? []}
	{#if localBranches.length === 0}
		<ProblemLoadingRepo
			{projectId}
			error="Create a local branch to use as the project target, then reopen the project."
		/>
	{:else}
		<ProjectSetup {projectId} {remoteBranches} {localBranches} {notice} />
	{/if}
{:else if remoteBranchesQuery.result.isError || localBranchesQuery.result.isError}
	<ProblemLoadingRepo
		{projectId}
		error="Could not load branches. Check the repository and reopen the project."
	/>
{/if}
