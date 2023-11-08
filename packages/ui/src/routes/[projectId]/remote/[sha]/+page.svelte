<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import ProjectHeader from '../../ProjectHeader.svelte';
	import RemoteBranchPreview from './RemoteBranchPreview.svelte';

	export let data: PageData;
	let {
		projectId,
		branchController,
		project,
		githubContextStore,
		remoteBranchStore,
		remoteBranchState
	} = data;

	$: branch = $remoteBranchStore?.find((b) => b.sha == $page.params.sha);
</script>

<div class="bg-color-3 flex h-full flex-grow flex-col overflow-y-auto overscroll-none">
	<ProjectHeader
		{projectId}
		projectTitle={$project?.title || ''}
		isGitHub={!!$githubContextStore}
		pageTitle={branch?.name}
	/>
	<div class="flex-grow px-8">
		{#if $remoteBranchState?.isLoading}
			<p>Loading...</p>
		{:else if $remoteBranchState?.isError}
			<p>Error...</p>
		{:else if branch}
			<RemoteBranchPreview {projectId} {branchController} {branch} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>
