<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import ProjectHeader from '../../ProjectHeader.svelte';
	import BranchLane from '../../components/BranchLane.svelte';

	export let data: PageData;
	let {
		projectId,
		branchController,
		project,
		githubContextStore,
		cloud,
		vbranchStore,
		vbranchesState,
		baseBranchStore
	} = data;

	$: branch = $vbranchStore?.find((b) => b.id == $page.params.branchId);
</script>

<div class="bg-color-3 flex h-full flex-grow flex-col overflow-y-auto overscroll-none">
	<ProjectHeader
		{projectId}
		projectTitle={$project?.title || ''}
		isGitHub={!!$githubContextStore}
		pageTitle={branch?.name}
	/>
	<div class="flex-grow px-8">
		{#if $vbranchesState.isLoading}
			<p>Loading...</p>
		{:else if $vbranchesState.isError}
			<p>Error...</p>
		{:else if $vbranchStore}
			{#if branch}
				<BranchLane
					{branch}
					{branchController}
					base={$baseBranchStore}
					{cloud}
					{projectId}
					maximized={true}
					cloudEnabled={false}
					projectPath=""
					readonly={true}
					githubContext={$githubContextStore}
				/>
			{:else}
				<p>Branch no longer exists</p>
			{/if}
		{/if}
	</div>
</div>
