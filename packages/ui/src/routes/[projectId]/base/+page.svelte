<script lang="ts">
	import type { PageData } from './$types';
	import BaseBranch from './BaseBranch.svelte';

	export let data: PageData;
	$: projectId = data.projectId;
	$: branchController = data.branchController;
	$: baseBranchService = data.baseBranchService;
	$: base$ = baseBranchService.base$;
	$: error$ = baseBranchService.error$;
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	<div
		class="flex max-w-4xl flex-col gap-y-6 overflow-visible rounded-lg px-5 py-4"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
		{#if $error$}
			<p>Error...</p>
		{:else if !$base$}
			<p>Loading...</p>
		{:else}
			<BaseBranch {projectId} base={$base$} {branchController} />
		{/if}
	</div>
</div>
