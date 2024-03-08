<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menuActions';
	import Navigation from '$lib/components/Navigation.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import { subscribe as menuSubscribe } from '$lib/menu';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/random';
	import { onDestroy, onMount } from 'svelte';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	export let data: LayoutData;

	$: projectService = data.projectService;
	$: branchController = data.branchController;
	$: githubService = data.githubService;
	$: vbranchService = data.vbranchService;
	$: branchesError$ = vbranchService.branchesError$;
	$: project$ = data.project$;
	$: branchService = data.branchService;
	$: userService = data.userService;
	$: projectId = data.projectId;

	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;
	$: baseError$ = baseBranchService.error$;
	$: gbBranchActive$ = data.gbBranchActive$;

	$: user$ = data.user$;

	let trayViewport: HTMLElement;
	let intervalId: any;
	handleMenuActions(data.projectId);

	// Once on load and every time the project id changes
	$: if (projectId) setupFetchInterval();

	function setupFetchInterval() {
		baseBranchService.fetchFromTarget();
		clearFetchInterval();
		intervalId = setInterval(() => baseBranchService.fetchFromTarget(), 15 * 60 * 1000);
	}

	function clearFetchInterval() {
		if (intervalId) clearInterval(intervalId);
	}

	$: if ($baseBranch$ === null) {
		goto(`/${projectId}/setup`, { replaceState: true });
	}

	onMount(() =>
		unsubscribe(
			menuSubscribe(data.projectId),
			hotkeys.on('Meta+Shift+S', () => syncToCloud(projectId))
		)
	);

	onDestroy(() => {
		clearFetchInterval();
	});
</script>

{#if !$project$}
	<p>Project not found!</p>
{:else if $baseError$}
	<ProblemLoadingRepo {projectService} {userService} project={$project$} error={$baseError$} />
{:else if $branchesError$}
	<ProblemLoadingRepo {projectService} {userService} project={$project$} error={$branchesError$} />
{:else if !$gbBranchActive$ && $baseBranch$}
	<NotOnGitButlerBranch
		{userService}
		{projectService}
		{branchController}
		project={$project$}
		baseBranch={$baseBranch$}
	/>
{:else if $baseBranch$}
	<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<div bind:this={trayViewport} class="flex flex-shrink">
			<Navigation
				{branchService}
				{baseBranchService}
				{branchController}
				project={$project$}
				user={$user$}
				{githubService}
				{projectService}
			/>
		</div>
		<div class="absolute h-4 w-full" data-tauri-drag-region></div>
		<slot />
	</div>
{:else if $baseBranch$ === undefined}
	loading...
{:else}
	<slot />
{/if}
