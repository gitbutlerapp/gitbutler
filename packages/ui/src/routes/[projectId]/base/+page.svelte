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

<div class="h-full flex-grow overflow-y-auto overscroll-none p-4">
	<div class="wrapper flex max-w-2xl flex-col gap-y-6 overflow-visible">
		{#if $error$}
			<p>Error...</p>
		{:else if !$base$}
			<p>Loading...</p>
		{:else}
			<BaseBranch {projectId} base={$base$} {branchController} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.wrapper {
		border: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		padding: var(--space-16);
		border-radius: var(--radius-m);
	}
</style>
