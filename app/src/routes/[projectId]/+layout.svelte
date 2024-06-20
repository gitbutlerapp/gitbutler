<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import { BranchService } from '$lib/branches/service';
	import { CommitDragActionsFactory } from '$lib/commits/dragActions';
	import History from '$lib/components/History.svelte';
	import NoBaseBranch from '$lib/components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSettingsMenuAction from '$lib/components/ProjectSettingsMenuAction.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import { HistoryService } from '$lib/history/history';
	import Navigation from '$lib/navigation/Navigation.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
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
		branchController,
		branchDragActionsFactory,
		commitDragActionsFactory,
		reorderDropzoneManagerFactory
	} = data);

	$: branchesError = vbranchService.branchesError;
	$: baseBranch = baseBranchService.base;
	$: baseError = baseBranchService.error;
	$: projectError = projectService.error;

	$: setContext(HistoryService, data.historyService);
	$: setContext(VirtualBranchService, vbranchService);
	$: setContext(BranchController, branchController);
	$: setContext(BranchService, branchService);
	$: setContext(BaseBranchService, baseBranchService);
	$: setContext(BaseBranch, baseBranch);
	$: setContext(Project, project);
	$: setContext(BranchDragActionsFactory, branchDragActionsFactory);
	$: setContext(CommitDragActionsFactory, commitDragActionsFactory);
	$: setContext(ReorderDropzoneManagerFactory, reorderDropzoneManagerFactory);

	const showHistoryView = persisted(false, 'showHistoryView');

	let intervalId: any;

	// Once on load and every time the project id changes
	$: if (projectId) setupFetchInterval();

	function setupFetchInterval() {
		baseBranchService.fetchFromRemotes();
		clearFetchInterval();
		const intervalMs = 15 * 60 * 1000; // 15 minutes
		intervalId = setInterval(async () => await baseBranchService.fetchFromRemotes(), intervalMs);
	}

	function clearFetchInterval() {
		if (intervalId) clearInterval(intervalId);
	}

	onMount(() => {
		const unsubscribe = listen<string>('menu://project/history/clicked', () => {
			$showHistoryView = !$showHistoryView;
		});

		return async () => {
			unsubscribe();
		};
	});

	const handleKeyDown = createKeybind({
		'$mod+Shift+H': () => {
			$showHistoryView = !$showHistoryView;
		}
	});

	onMount(() => {
		return unsubscribe(
			events.on('openHistory', () => {
				$showHistoryView = true;
			})
		);
	});

	onDestroy(() => clearFetchInterval());
</script>

<svelte:window on:keydown={handleKeyDown} />

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
			{#if $showHistoryView}
				<History on:hide={() => ($showHistoryView = false)} />
			{/if}
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
