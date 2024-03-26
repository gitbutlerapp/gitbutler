<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import { BranchService } from '$lib/branches/service';
	import Navigation from '$lib/components/Navigation.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranchService, NoDefaultTarget } from '$lib/vbranches/branchStoresCache';
	import { BaseBranch } from '$lib/vbranches/types';
	import { onDestroy, onMount, setContext } from 'svelte';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	export let data: LayoutData;

	$: ({
		vbranchService,
		project,
		projectId,
		projectService,
		baseBranchService,
		gbBranchActive$,
		branchService,
		branchController,
		menuBarController
	} = data);

	$: branchesError = vbranchService.branchesError;
	$: baseBranch = baseBranchService.base;
	$: baseError = baseBranchService.error;
	$: projectError = projectService.error$;

	$: setContext(BranchController, branchController);
	$: setContext(BranchService, branchService);
	$: setContext(BaseBranchService, baseBranchService);
	$: setContext(BaseBranch, baseBranch);
	$: setContext(Project, project);

	let intervalId: any;

	// Once on load and every time the project id changes
	$: if (projectId) setupFetchInterval();

	$: menuBarController.setProjectId(projectId);

	// We need to setup the project if default target not set
	$: if ($baseError instanceof NoDefaultTarget) {
		goto(`/${projectId}/setup`, { replaceState: true });
	}

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

		return () => {
			unsubscribe(cloudSyncSubscription)();
			menuBarController.setProjectId(undefined);
		};
	});

	onDestroy(() => clearFetchInterval());
</script>

<!-- forces components to be recreated when projectId changes -->
{#key projectId}
	{#if !project}
		<p>Project not found!</p>
	{:else if $baseError instanceof NoDefaultTarget}
		<!-- Note that this requires the redirect above to work -->
		<slot />
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
