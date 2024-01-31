<script lang="ts">
	import BaseBranch from '$lib/components/BaseBranch.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import type { PageData } from './$types';

	export let data: PageData;
	$: projectId = data.projectId;
	$: branchController = data.branchController;
	$: baseBranchService = data.baseBranchService;
	$: base$ = baseBranchService.base$;
	$: error$ = baseBranchService.error$;
	$: project$ = data.project$;
</script>

<ScrollableContainer wide>
	<div class="card">
		{#if $error$}
			<p>Error...</p>
		{:else if !$base$}
			<p>Loading...</p>
		{:else}
			<BaseBranch {projectId} base={$base$} {branchController} projectPath={$project$.path} />
		{/if}
	</div>
</ScrollableContainer>

<style lang="postcss">
	.card {
		margin: var(--space-16);
		padding: var(--space-16);
		max-width: 50rem;
	}
</style>
