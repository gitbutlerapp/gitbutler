<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menuActions';
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import { subscribe as menuSubscribe } from '$lib/menu';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/random';
	import { BranchController } from '$lib/vbranches/branchController';
	import { onDestroy, onMount, setContext } from 'svelte';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	export let data: LayoutData;

	$: vbranchService = data.vbranchService;
	$: branchesError$ = vbranchService.branchesError$;
	$: project$ = data.project$;
	$: branchService = data.branchService;
	$: projectId = data.projectId;
	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;
	$: baseError$ = baseBranchService.error$;
	$: gbBranchActive$ = data.gbBranchActive$;
	$: user$ = data.user$;

	$: setContext(BranchController, data.branchController);

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

	onMount(() => {
		return unsubscribe(
			menuSubscribe(data.projectId),
			hotkeys.on('Meta+Shift+S', () => syncToCloud(projectId))
		);
	});

	onDestroy(() => {
		clearFetchInterval();
	});

	$: if (data) {
		setContext('hello', data.projectId);
	}
</script>

{#if !$project$}
	<p>Project not found!</p>
{:else if $baseBranch$ === null}
	<!-- Be careful, this works because of the redirect above -->
	<slot />
{:else if $baseError$}
	<ProblemLoadingRepo project={$project$} error={$baseError$} />
{:else if $branchesError$}
	<ProblemLoadingRepo project={$project$} error={$branchesError$} />
{:else if !$gbBranchActive$ && $baseBranch$}
	<NotOnGitButlerBranch project={$project$} baseBranch={$baseBranch$} />
{:else if $baseBranch$}
	<div class="view-wrap" role="group" on:dragover|preventDefault>
		<Navigation {branchService} {baseBranchService} project={$project$} user={$user$} />
		<slot />
	</div>
{:else}
	<FullscreenLoading />
{/if}

<style>
	.view-wrap {
		position: relative;
		display: flex;
		width: 100%;
	}
</style>
