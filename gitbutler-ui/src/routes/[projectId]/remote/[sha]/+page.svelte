<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import RemoteBranchPreview from './RemoteBranchPreview.svelte';

	export let data: PageData;
	$: projectId = data.projectId;
	$: branchController = data.branchController;
	$: remoteBranchService = data.remoteBranchService;
	$: branches$ = remoteBranchService.branches$;
	$: error$ = remoteBranchService.branchesError$;

	$: branch = $branches$?.find((b) => b.sha == $page.params.sha);
</script>

<div class="h-full max-w-xl flex-grow flex-col overflow-y-auto overscroll-none p-4">
	<div
		class="rounded-lg border"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
		{#if $error$}
			<p>Error...</p>
		{:else if !$branches$}
			<p>Loading...</p>
		{:else if branch}
			<RemoteBranchPreview {projectId} {branchController} {branch} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>
