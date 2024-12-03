<script lang="ts">
	import { projectCloudSync } from '$lib/backend/projectCloudSync.svelte';
	import { Project, ProjectService } from '$lib/backend/projects';
	import { TemplateService } from '$lib/backend/templateService';
	import FileMenuAction from '$lib/barmenuActions/FileMenuAction.svelte';
	import ProjectSettingsMenuAction from '$lib/barmenuActions/ProjectSettingsMenuAction.svelte';
	import { BaseBranch, NoDefaultTarget } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { CloudBranchCreationService } from '$lib/branch/cloudBranchCreationService';
	import { BranchListingService, CombinedBranchListingService } from '$lib/branches/branchListing';
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import { GitBranchService } from '$lib/branches/gitBranch';
	import { CommitDragActionsFactory } from '$lib/commits/dragActions';
	import { CommitService } from '$lib/commits/service';
	import NoBaseBranch from '$lib/components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import { showHistoryView } from '$lib/config/config';
	import { cloudFunctionality } from '$lib/config/uiFeatureFlags';
	import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory';
	import { octokitFromAccessToken } from '$lib/forge/github/octokit';
	import { createForgeStore } from '$lib/forge/interface/forge';
	import { createForgeListingServiceStore } from '$lib/forge/interface/forgeListingService';
	import { createForgePrServiceStore } from '$lib/forge/interface/forgePrService';
	import History from '$lib/history/History.svelte';
	import { HistoryService } from '$lib/history/history';
	import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
	import { ModeService } from '$lib/modes/service';
	import Navigation from '$lib/navigation/Navigation.svelte';
	import { UncommitedFilesWatcher } from '$lib/uncommitedFiles/watcher';
	import { debounce } from '$lib/utils/debounce';
	import { BranchController } from '$lib/vbranches/branchController';
	import { UpstreamIntegrationService } from '$lib/vbranches/upstreamIntegrationService';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { CloudBranchesService } from '@gitbutler/shared/cloud/stacks/service';
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { DesktopRoutesService, getRoutesService } from '@gitbutler/shared/sharedRoutes';
	import { onDestroy, setContext, type Snippet } from 'svelte';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const {
		vbranchService,
		project,
		projectId,
		projectsService,
		baseBranchService,
		branchListingService,
		modeService,
		userService,
		fetchSignal,
		posthog,
		projectMetrics
	} = $derived(data);

	const branchesError = $derived(vbranchService.branchesError);
	const baseBranch = $derived(baseBranchService.base);
	const repoInfo = $derived(baseBranchService.repo);
	const forkInfo = $derived(baseBranchService.pushRepo);
	const user = $derived(userService.user);
	const accessToken = $derived($user?.github_access_token);
	const baseError = $derived(baseBranchService.error);
	const projectError = $derived(projectsService.error);

	$effect.pre(() => {
		setContext(HistoryService, data.historyService);
		setContext(VirtualBranchService, data.vbranchService);
		setContext(BranchController, data.branchController);
		setContext(BaseBranchService, data.baseBranchService);
		setContext(CommitService, data.commitService);
		setContext(TemplateService, data.templateService);
		setContext(BaseBranch, baseBranch);
		setContext(Project, project);
		setContext(BranchDragActionsFactory, data.branchDragActionsFactory);
		setContext(CommitDragActionsFactory, data.commitDragActionsFactory);
		setContext(StackingReorderDropzoneManagerFactory, data.stackingReorderDropzoneManagerFactory);
		setContext(GitBranchService, data.gitBranchService);
		setContext(BranchListingService, data.branchListingService);
		setContext(ModeService, data.modeService);
		setContext(UncommitedFilesWatcher, data.uncommitedFileWatcher);
		setContext(UpstreamIntegrationService, data.upstreamIntegrationService);
		setContext(ProjectService, data.projectService);

		// Cloud related services
		setContext(SyncedSnapshotService, data.syncedSnapshotService);
		setContext(CloudBranchesService, data.cloudBranchesService);
		setContext(CloudBranchCreationService, data.cloudBranchCreationService);
	});

	const routesService = getRoutesService();
	$effect(() => {
		if (routesService instanceof DesktopRoutesService) {
			routesService.currentProjectId.set(projectId);
		}
	});

	let intervalId: any;

	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);
	const forgeFactory = $derived(new DefaultForgeFactory(octokit, posthog, projectMetrics));
	const baseBranchName = $derived($baseBranch?.shortName);

	const listServiceStore = createForgeListingServiceStore(undefined);
	const forgeStore = createForgeStore(undefined);
	const prService = createForgePrServiceStore(undefined);

	$effect.pre(() => {
		const combinedBranchListingService = new CombinedBranchListingService(
			data.branchListingService,
			listServiceStore,
			projectId
		);

		setContext(CombinedBranchListingService, combinedBranchListingService);
	});

	// Refresh base branch if git fetch event is detected.
	const mode = $derived(modeService.mode);
	const head = $derived(modeService.head);

	// TODO: can we eliminate the need to debounce?
	const fetch = $derived(fetchSignal.event);
	const debouncedBaseBranchRefresh = debounce(async () => await baseBranchService.refresh(), 500);
	$effect(() => {
		if ($fetch || $head) debouncedBaseBranchRefresh();
	});

	// TODO: can we eliminate the need to debounce?
	const debouncedRemoteBranchRefresh = debounce(
		async () => await branchListingService?.refresh(),
		500
	);

	$effect(() => {
		if ($baseBranch || $head || $fetch) debouncedRemoteBranchRefresh();
	});

	$effect(() => {
		const forge =
			$repoInfo && baseBranchName
				? forgeFactory.build($repoInfo, baseBranchName, $forkInfo)
				: undefined;
		const ghListService = forge?.listService();
		listServiceStore.set(ghListService);
		forgeStore.set(forge);
		prService.set(forge ? forge.prService() : undefined);
		posthog.setPostHogRepo($repoInfo);
		return () => {
			posthog.setPostHogRepo(undefined);
		};
	});

	// Once on load and every time the project id changes
	$effect(() => {
		if (projectId) {
			setupFetchInterval();
		} else {
			goto('/onboarding');
		}
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

	const appState = getContext(AppState);
	const cloudProjectService = getContext(CloudProjectService);
	const httpClient = getContext(HttpClient);

	$effect(() => {
		if (!$cloudFunctionality) return;

		projectCloudSync(
			appState,
			data.projectsService,
			data.projectService,
			cloudProjectService,
			httpClient
		);
	});

	onDestroy(() => {
		clearFetchInterval();
	});
</script>

<!-- forces components to be recreated when projectId changes -->
{#key projectId}
	<ProjectSettingsMenuAction />
	<FileMenuAction />

	{#if !project}
		<p>Project not found!</p>
	{:else if $baseError instanceof NoDefaultTarget}
		<NoBaseBranch />
	{:else if $baseError}
		<ProblemLoadingRepo error={$baseError} />
	{:else if $branchesError}
		<ProblemLoadingRepo error={$branchesError} />
	{:else if $projectError}
		<ProblemLoadingRepo error={$projectError} />
	{:else if $baseBranch}
		{#if $mode?.type === 'OpenWorkspace' || $mode?.type === 'Edit'}
			<div class="view-wrap" role="group" ondragover={(e) => e.preventDefault()}>
				<Navigation />
				{#if $showHistoryView}
					<History onHide={() => ($showHistoryView = false)} />
				{/if}
				{@render children()}
			</div>
		{:else if $mode?.type === 'OutsideWorkspace'}
			<NotOnGitButlerBranch baseBranch={$baseBranch} />
		{/if}
	{/if}
{/key}

<style>
	.view-wrap {
		position: relative;
		display: flex;
		width: 100%;
	}
</style>
