<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$components/ProjectSetup.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { inject } from '@gitbutler/core/context';

	const { projectId }: { projectId: string } = $props();
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const remoteBranchesQuery = $derived(baseBranchService.remoteBranches(projectId));
</script>

{#if remoteBranchesQuery.result.isLoading}
	<!--TODO: Add project id -->
	<FullviewLoading />
{:else if remoteBranchesQuery.result.isSuccess}
	{@const remoteBranches = remoteBranchesQuery.response}
	{#if !remoteBranches || remoteBranches.length === 0}
		<ProblemLoadingRepo
			{projectId}
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup {projectId} {remoteBranches} />
	{/if}
{:else if remoteBranchesQuery.result.isError}
	<ProblemLoadingRepo
		{projectId}
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/if}
