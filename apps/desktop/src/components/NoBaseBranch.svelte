<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$components/ProjectSetup.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';

	const project = getContext(Project);
	const projectId = $derived(project.id);
	const baseBranchService = getContext(BaseBranchService);
	const remoteBranchesResponse = $derived(baseBranchService.remoteBranches(projectId));
</script>

{#if remoteBranchesResponse.current.isLoading}
	<!--TODO: Add project id -->
	<FullviewLoading />
{:else if remoteBranchesResponse.current.isSuccess}
	{@const remoteBranches = remoteBranchesResponse.current.data}
	{#if remoteBranches.length === 0}
		<ProblemLoadingRepo
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup {remoteBranches} />
	{/if}
{:else if remoteBranchesResponse.current.isError}
	<ProblemLoadingRepo
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/if}
