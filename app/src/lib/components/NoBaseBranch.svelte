<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$lib/components/ProjectSetup.svelte';
	import { getContext } from '$lib/utils/context';
	import { getRemoteBranches } from '$lib/vbranches/baseBranch';

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
