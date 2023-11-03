<script lang="ts">
	import type { PageData } from './$types';
	import BaseBranch from './BaseBranch.svelte';

	export let data: PageData;
	let { projectId, branchController, baseBranchStore, baseBranchesState } = data;
</script>

<div class="bg-color-3 h-full flex-grow overflow-y-auto overscroll-none">
	<div class="mx-auto flex max-w-xl flex-col gap-y-6 overflow-visible p-8">
		{#if $baseBranchesState.isLoading}
			<p>Loading...</p>
		{:else if $baseBranchesState.isError}
			<p>Error...</p>
		{:else if $baseBranchStore}
			<BaseBranch {projectId} base={$baseBranchStore} {branchController} />
		{/if}
	</div>
</div>
