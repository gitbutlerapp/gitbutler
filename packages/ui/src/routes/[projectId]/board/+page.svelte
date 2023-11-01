<script lang="ts">
	import type { PageData } from './$types';
	import Board from './Board.svelte';

	export let data: PageData;
	let {
		projectId,
		cloud,
		baseBranchStore,
		baseBranchesState,
		branchController,
		branchesWithContent,
		branchesState,
		githubContextStore,
		project
	} = data;
</script>

<Board
	{branchController}
	{projectId}
	{cloud}
	base={$baseBranchStore}
	branches={$branchesWithContent}
	projectPath={$project?.path}
	cloudEnabled={$project?.api?.sync || false}
	githubContext={$githubContextStore}
	branchesState={$branchesState}
	baseBranchState={$baseBranchesState}
/>
