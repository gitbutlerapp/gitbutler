<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import { BranchService } from '$lib/branches/service';
	import Navigation from '$lib/components/Navigation.svelte';
	import NoBaseBranch from '$lib/components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSettingsMenuAction from '$lib/components/ProjectSettingsMenuAction.svelte';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { BaseBranchService, NoDefaultTarget } from '$lib/vbranches/baseBranch';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onDestroy, onMount, setContext } from 'svelte';
	import type { LayoutData } from './$types';

	export let data: LayoutData;

	$: ({
		vbranchService,
		project,
		projectId,
		projectService,
		baseBranchService,
		gbBranchActive$,
		branchService,
		branchController
	} = data);

	$: branchesError = vbranchService.branchesError;
	$: baseBranch = baseBranchService.base;
	$: baseError = baseBranchService.error;
	$: projectError = projectService.error;

	$: setContext(VirtualBranchService, vbranchService);
	$: setContext(BranchController, branchController);
	$: setContext(BranchService, branchService);
	$: setContext(BaseBranchService, baseBranchService);
	$: setContext(BaseBranch, baseBranch);
	$: setContext(Project, project);

	let intervalId: any;

	// Once on load and every time the project id changes
	$: if (projectId) setupFetchInterval();

	function setupFetchInterval() {
		baseBranchService.fetchFromTarget();
		clearFetchInterval();
		const intervalMs = 15 * 60 * 1000; // 15 minutes
		intervalId = setInterval(() => baseBranchService.fetchFromTarget(), intervalMs);
	}

	function clearFetchInterval() {
		if (intervalId) clearInterval(intervalId);
	}

	onMount(() => {
		const cloudSyncSubscription = hotkeys.on('Meta+Shift+S', () => syncToCloud(projectId));

		return unsubscribe(cloudSyncSubscription);
	});

	onDestroy(() => clearFetchInterval());
</script>

<!-- forces components to be recreated when projectId changes -->
{#key projectId}
	<ProjectSettingsMenuAction />

	{#if !project}
		<p>Project not found!</p>
	{:else if $baseError instanceof NoDefaultTarget}
		<!-- Note that this requires the redirect above to work -->
		<NoBaseBranch />
	{:else if $baseError}
		<ProblemLoadingRepo error={$baseError} />
	{:else if $branchesError}
		<ProblemLoadingRepo error={$branchesError} />
	{:else if $projectError}
		<ProblemLoadingRepo error={$projectError} />
	{:else if !$gbBranchActive$ && $baseBranch}
		<NotOnGitButlerBranch baseBranch={$baseBranch} />
	{:else if $baseBranch}
		<div class="view-wrap" role="group" on:dragover|preventDefault>
			<Navigation />
			<slot />
		</div>
	{/if}
{/key}

<style>
	.view-wrap {
		position: relative;
		display: flex;
		width: 100%;
	}
</style>
