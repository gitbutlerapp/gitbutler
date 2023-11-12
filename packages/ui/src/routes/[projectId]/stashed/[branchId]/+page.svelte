<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import ProjectHeader from '../../ProjectHeader.svelte';
	import BranchLane from '../../components/BranchLane.svelte';

	export let data: PageData;
	let {
		projectId,
		branchController,
		githubContextStore,
		cloud,
		vbranchStore,
		vbranchesState,
		baseBranchStore
	} = data;

	$: branch = $vbranchStore?.find((b) => b.id == $page.params.branchId);
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	<div
		class="flex min-w-min max-w-4xl flex-col gap-y-6 overflow-visible rounded-lg px-5 py-4"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
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
