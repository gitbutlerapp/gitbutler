<script lang="ts">
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$lib/components/ProjectSetup.svelte';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';
	import type { PageData } from './$types';

	export let data: PageData;

	$: ({ projectId, project$ } = data);
</script>

{#await getRemoteBranches(projectId)}
	<!--TODO: Add project id -->
	<FullviewLoading />
{:then remoteBranches}
	{#if remoteBranches.length == 0}
		<ProblemLoadingRepo
			project={$project$}
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{:else}
		<ProjectSetup project={$project$} {remoteBranches} />
	{/if}
{:catch}
	<ProblemLoadingRepo
		project={$project$}
		error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
	/>
{/await}
