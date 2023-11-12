<script lang="ts">
	import type { PageData } from './$types';
	import BaseBranch from './BaseBranch.svelte';

	export let data: PageData;
	let { projectId, branchController, baseBranchStore, baseBranchesState } = data;
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	<div
		class="flex max-w-4xl flex-col gap-y-6 overflow-visible rounded-lg px-5 py-4"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
		{#if $baseBranchesState.isLoading}
			<p>Loading...</p>
		{:else if $baseBranchesState.isError}
			<p>Error...</p>
		{:else if $baseBranchStore}
			<BaseBranch {projectId} base={$baseBranchStore} {branchController} />
		{/if}
	</div>
</div>
