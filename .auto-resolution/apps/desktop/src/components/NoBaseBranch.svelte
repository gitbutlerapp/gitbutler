<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$components/ProjectSetup.svelte';
	import { getRemoteBranches } from '$lib/baseBranch/baseBranchService';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';

	const project = getContext(Project);
</script>

{#await getRemoteBranches(project.id)}
	<!--TODO: Add project id -->
	<FullviewLoading />
{:then remoteBranches}
	{#if remoteBranches.length === 0}
		<ProblemLoadingRepo
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup {remoteBranches} />
	{/if}
{:catch}
	<ProblemLoadingRepo
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/await}
