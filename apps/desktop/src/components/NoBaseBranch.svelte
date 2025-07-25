<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$components/ProjectSetup.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { inject } from '@gitbutler/shared/context';

	const { projectId }: { projectId: string } = $props();
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const remoteBranchesResponse = $derived(baseBranchService.remoteBranches(projectId));
</script>

{#if remoteBranchesResponse.current.isLoading}
	<!--TODO: Add project id -->
	<FullviewLoading />
{:else if remoteBranchesResponse.current.isSuccess}
	{@const remoteBranches = remoteBranchesResponse.current.data}
	{#if remoteBranches.length === 0}
		<ProblemLoadingRepo
			{projectId}
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup {projectId} {remoteBranches} />
	{/if}
{:else if remoteBranchesResponse.current.isError}
	<ProblemLoadingRepo
		{projectId}
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/if}
