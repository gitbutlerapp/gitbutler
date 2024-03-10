<script lang="ts">
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$lib/components/ProjectSetup.svelte';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';
	import type { PageData } from './$types';

	export let data: PageData;

	$: branchController = data.branchController;
	$: baseBranchService = data.baseBranchService;
	$: projectService = data.projectService;
	$: userService = data.userService;
	$: authService = data.authService;
	$: projectId = data.projectId;
	$: project$ = data.project$;
</script>

{#await getRemoteBranches(projectId)}
	<!--TODO: Add project id -->
	<FullscreenLoading />
{:then remoteBranches}
	{#if remoteBranches.length == 0}
		<ProblemLoadingRepo
			{userService}
			{projectService}
			project={$project$}
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup
			project={$project$}
			{projectService}
			{authService}
			{baseBranchService}
			{branchController}
			{userService}
			{remoteBranches}
		/>
	{/if}
{:catch}
	<ProblemLoadingRepo
		{userService}
		{projectService}
		project={$project$}
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/await}
