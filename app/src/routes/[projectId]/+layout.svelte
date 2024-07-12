<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { Project } from '$lib/backend/projects';
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import { BranchService, createBranchServiceStore } from '$lib/branches/service';
	import { CommitDragActionsFactory } from '$lib/commits/dragActions';
	import NoBaseBranch from '$lib/components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSettingsMenuAction from '$lib/components/ProjectSettingsMenuAction.svelte';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import History from '$lib/history/History.svelte';
	import { HistoryService } from '$lib/history/history';
	import { octokitFromAccessToken } from '$lib/hostedServices/github/octokit';
	import { DefaultHostedGitServiceFactory } from '$lib/hostedServices/hostedGitServiceFactory';
	import { createHostedGitListingServiceStore } from '$lib/hostedServices/interface/hostedGitListingService';
	import { createHostedGitServiceStore } from '$lib/hostedServices/interface/hostedGitService';
	import Navigation from '$lib/navigation/Navigation.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { BaseBranchService, NoDefaultTarget } from '$lib/vbranches/baseBranch';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onDestroy, onMount, setContext, type Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const {
		vbranchService,
		project,
		projectId,
		projectService,
		baseBranchService,
		remoteBranchService,
		gbBranchActive$,
		userService
	} = $derived(data);

	const branchesError = $derived(vbranchService.branchesError);
	const baseBranch = $derived(baseBranchService.base);
	const remoteUrl = $derived($baseBranch?.remoteUrl);
	const user = $derived(userService.user);
	const accessToken = $derived($user?.github_access_token);
	const baseError = $derived(baseBranchService.error);
	const projectError = $derived(projectService.error);

	$effect.pre(() => {
		setContext(HistoryService, data.historyService);
		setContext(VirtualBranchService, data.vbranchService);
		setContext(BranchController, data.branchController);
		setContext(BaseBranchService, data.baseBranchService);
		setContext(BaseBranch, baseBranch);
		setContext(Project, project);
		setContext(BranchDragActionsFactory, data.branchDragActionsFactory);
		setContext(CommitDragActionsFactory, data.commitDragActionsFactory);
		setContext(ReorderDropzoneManagerFactory, data.reorderDropzoneManagerFactory);
	});

	let intervalId: any;

	const showHistoryView = persisted(false, 'showHistoryView');

	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);
	const hostedGitServiceFactory = $derived(new DefaultHostedGitServiceFactory(octokit));
	const repoInfo = $derived(remoteUrl ? parseRemoteUrl(remoteUrl) : undefined);
	const githubRepoServiceStore = createHostedGitServiceStore(undefined);
	const branchServiceStore = createBranchServiceStore(undefined);
	const listServiceStore = createHostedGitListingServiceStore(undefined);

	$effect.pre(() => {
		if (repoInfo) {
			const hostedGitService = hostedGitServiceFactory.build(repoInfo);
			const ghListService = hostedGitService?.listService();
			listServiceStore.set(ghListService);
			githubRepoServiceStore.set(hostedGitService);

			branchServiceStore.set(new BranchService(vbranchService, remoteBranchService, ghListService));
		}
	});

	// Once on load and every time the project id changes
	$effect(() => {
		if (projectId) setupFetchInterval();
	});

	function setupFetchInterval() {
		baseBranchService.fetchFromRemotes();
		clearFetchInterval();
		const intervalMs = 15 * 60 * 1000; // 15 minutes
		intervalId = setInterval(async () => {
			await baseBranchService.fetchFromRemotes();
		}, intervalMs);
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
		<div class="view-wrap" role="group" ondragover={(e) => e.preventDefault()}>
			<Navigation />
			{#if $showHistoryView}
				<History on:hide={() => ($showHistoryView = false)} />
			{/if}
			{@render children()}
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
