<script lang="ts">
	import { goto } from '$app/navigation';
	import Chrome from '$components/Chrome.svelte';
	import FileMenuAction from '$components/FileMenuAction.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import History from '$components/History.svelte';
	import MetricsReporter from '$components/MetricsReporter.svelte';
	import Navigation from '$components/Navigation.svelte';
	import NoBaseBranch from '$components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$components/ProblemLoadingRepo.svelte';
	import ProjectSettingsMenuAction from '$components/ProjectSettingsMenuAction.svelte';
	import TryV3Modal from '$components/TryV3Modal.svelte';
	import IrcPopups from '$components/v3/IrcPopups.svelte';
	import NotOnGitButlerBranchV3 from '$components/v3/NotOnGitButlerBranch.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { GitBranchService } from '$lib/branches/gitBranch';
	import { VirtualBranchService } from '$lib/branches/virtualBranchService';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { showHistoryView } from '$lib/config/config';
	import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
	import { UncommitedFilesWatcher } from '$lib/files/watcher';
	import { FocusManager } from '$lib/focus/focusManager.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { GitHubClient } from '$lib/forge/github/githubClient';
	import { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
	import { GitLabState } from '$lib/forge/gitlab/gitlabState.svelte';
	import { BrToPrService } from '$lib/forge/shared/prFooter';
	import { TemplateService } from '$lib/forge/templateService';
	import { HistoryService } from '$lib/history/history';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
	import { ModeService } from '$lib/mode/modeService';
	import { showError, showInfo } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { projectCloudSync } from '$lib/project/projectCloudSync.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { getSecretsService } from '$lib/secrets/secretsService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UpstreamIntegrationService } from '$lib/upstream/upstreamIntegrationService';
	import { debounce } from '$lib/utils/debounce';
	import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { onDestroy, setContext, type Snippet } from 'svelte';
	import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
	import type { LayoutData } from './$types';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const { project, projectId, projectsService, userService, fetchSignal, posthog, projectMetrics } =
		$derived(data);

	const baseBranchService = getContext(BaseBranchService);
	const repoInfoResponse = $derived(baseBranchService.repo(projectId));
	const repoInfo = $derived(repoInfoResponse.current.data);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchResponse.current.data);
	const pushRepoResponse = $derived(baseBranchService.pushRepo(projectId));
	const forkInfo = $derived(pushRepoResponse.current.data);
	const baseError = $derived(baseBranchResponse.current.error);
	const baseBranchName = $derived(baseBranch?.shortName);
	const branchService = getContext(BranchService);

	const stackService = getContext(StackService);
	const modeService = $derived(new ModeService(projectId, stackService));
	$effect.pre(() => {
		setContext(ModeService, modeService);
	});

	const vbranchService = $derived(
		new VirtualBranchService(projectId, projectMetrics, modeService, branchService)
	);

	const secretService = getSecretsService();
	const gitLabState = $derived(new GitLabState(secretService, repoInfo, projectId));
	$effect.pre(() => {
		setContext(GitLabState, gitLabState);
	});
	const gitLabClient = getContext(GitLabClient);
	$effect.pre(() => {
		gitLabClient.set(gitLabState);
	});

	const branchesError = $derived(vbranchService.branchesError);
	const user = $derived(userService.user);
	const accessToken = $derived($user?.github_access_token);

	const gitHubClient = getContext(GitHubClient);
	$effect.pre(() => gitHubClient.setToken(accessToken));
	$effect.pre(() => gitHubClient.setRepo({ owner: repoInfo?.owner, repo: repoInfo?.name }));

	const projectError = $derived(projectsService.error);
	const projects = $derived(projectsService.projects);

	const cloudBranchService = getContext(CloudBranchService);
	const cloudProjectService = getContext(CloudProjectService);
	const latestBranchLookupService = getContext(LatestBranchLookupService);

	$effect.pre(() => {
		const upstreamIntegrationService = new UpstreamIntegrationService(
			project,
			vbranchService,
			cloudBranchService,
			cloudProjectService,
			latestBranchLookupService
		);
		setContext(UpstreamIntegrationService, upstreamIntegrationService);
	});

	$effect.pre(() => {
		const stackingReorderDropzoneManagerFactory = new StackingReorderDropzoneManagerFactory(
			projectId,
			stackService
		);

		setContext(StackingReorderDropzoneManagerFactory, stackingReorderDropzoneManagerFactory);
	});

	$effect.pre(() => {
		setContext(HistoryService, data.historyService);
		setContext(TemplateService, data.templateService);
		setContext(BaseBranch, baseBranch);
		setContext(Project, project);
		setContext(GitBranchService, data.gitBranchService);
		setContext(UncommitedFilesWatcher, data.uncommitedFileWatcher);
		setContext(ProjectService, data.projectService);
		setContext(VirtualBranchService, vbranchService);

		// Cloud related services
		setContext(SyncedSnapshotService, data.syncedSnapshotService);
		setContext(StackPublishingService, data.stackPublishingService);
	});

	const focusManager = new FocusManager();
	setContext(FocusManager, focusManager);

	let intervalId: any;

	const forgeFactory = getContext(DefaultForgeFactory);

	// Refresh base branch if git fetch event is detected.
	const mode = $derived(modeService.mode);
	const head = $derived(modeService.head);

	// TODO: can we eliminate the need to debounce?
	const fetch = $derived(fetchSignal.event);
	const debouncedBaseBranchRefresh = debounce(async () => {
		await baseBranchService.refreshBaseBranch(projectId);
	}, 500);
	$effect(() => {
		if ($fetch || $head) debouncedBaseBranchRefresh();
	});

	// TODO: can we eliminate the need to debounce?
	const debouncedRemoteBranchRefresh = debounce(
		async () => await branchService.refresh(projectId),
		500
	);

	$effect(() => {
		if (baseBranch || $head || $fetch) debouncedRemoteBranchRefresh();
	});

	const gitlabConfigured = $derived(gitLabState.configured);

	$effect(() => {
		forgeFactory.setConfig({
			repo: repoInfo,
			pushRepo: forkInfo,
			baseBranch: baseBranchName,
			githubAuthenticated: !!$user?.github_access_token,
			gitlabAuthenticated: !!$gitlabConfigured,
			forgeOverride: $projects?.find((project) => project.id === projectId)?.forge_override
		});
	});

	$effect(() => {
		posthog.setPostHogRepo(repoInfo);
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

	// TODO(mattias): This is an ugly hack, fix it somehow?
	// I want to flush project metrics to local storage before e.g. switching
	// to a different project. Since `projectMetrics` is defined in layout.ts
	// we get no heads up when it is about to change, and reactively updated
	// in this scope through `LayoutData`. Even at time of unMount in e.g.
	// metrics reporter it seems as if the projectMetrics variable is already
	// referencing the new instance.
	let lastProjectMetrics: ProjectMetrics | undefined;
	$effect(() => {
		if (lastProjectMetrics) {
			lastProjectMetrics.saveToLocalStorage();
		}
		lastProjectMetrics = projectMetrics;
		projectMetrics.loadFromLocalStorage();
	});

	async function fetchRemoteForProject() {
		await baseBranchService.fetchFromRemotes(projectId, 'auto');
	}

	function setupFetchInterval() {
		const autoFetchIntervalMinutes = $settingsStore?.fetch.autoFetchIntervalMinutes || 15;
		if (autoFetchIntervalMinutes < 0) {
			return;
		}
		fetchRemoteForProject();
		clearFetchInterval();
		const intervalMs = autoFetchIntervalMinutes * 60 * 1000; // 15 minutes
		intervalId = setInterval(async () => {
			await fetchRemoteForProject();
		}, intervalMs);
	}

	function clearFetchInterval() {
		if (intervalId) clearInterval(intervalId);
	}

	const httpClient = getContext(HttpClient);

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const webRoutesService = getContext(WebRoutesService);
	const brToPrService = new BrToPrService(
		webRoutesService,
		cloudProjectService,
		latestBranchLookupService,
		forgeFactory
	);
	setContext(BrToPrService, brToPrService);

	$effect(() => {
		projectCloudSync(data.projectsService, data.projectService, httpClient);
	});

	onDestroy(() => {
		clearFetchInterval();
	});

	async function setActiveProjectOrRedirect() {
		// Optimistically assume the project is viewable
		try {
			const is_first_window_on_project = await projectsService.setActiveProject(projectId);
			if (!is_first_window_on_project) {
				showInfo(
					'Just FYI, this project is already open in another window',
					'There might be some unexpected behavior if you open it in multiple windows'
				);
			}
		} catch (error: unknown) {
			showError('Failed to set the project active', error);
		}
	}

	$effect(() => {
		setActiveProjectOrRedirect();
	});
</script>

<TryV3Modal />

<!-- forces components to be recreated when projectId changes -->
{#key projectId}
	<ProjectSettingsMenuAction />
	<FileMenuAction />

	{#if !project}
		<p>Project not found!</p>
	{:else if baseBranchResponse.current.isLoading && !baseBranch}
		<FullviewLoading />
	{:else if !baseBranch}
		<NoBaseBranch />
	{:else if baseError}
		<ProblemLoadingRepo error={baseError} />
	{:else if $branchesError}
		<ProblemLoadingRepo error={$branchesError} />
	{:else if $projectError}
		<ProblemLoadingRepo error={$projectError} />
	{:else if baseBranch}
		{#if $mode?.type === 'OpenWorkspace' || $mode?.type === 'Edit'}
			<div class="view-wrap" role="group" ondragover={(e) => e.preventDefault()}>
				{#if $settingsStore?.featureFlags.v3}
					<Chrome {projectId} sidebarDisabled={$mode?.type === 'Edit'}>
						{@render children()}
					</Chrome>
				{:else}
					<Navigation {projectId} />
					{@render children()}
				{/if}
				{#if $showHistoryView}
					<History onHide={() => ($showHistoryView = false)} />
				{/if}
			</div>
		{:else if $mode?.type === 'OutsideWorkspace' && !$settingsStore?.featureFlags.v3}
			<NotOnGitButlerBranch {baseBranch} />
		{:else if $mode?.type === 'OutsideWorkspace' && $settingsStore?.featureFlags.v3}
			<NotOnGitButlerBranchV3 {baseBranch} />
		{/if}
	{/if}
{/key}

{#if $settingsStore?.featureFlags.v3}
	<IrcPopups />
{/if}

<!-- Mounting metrics reporter in the board ensures dependent services are subscribed to. -->
<MetricsReporter {projectId} {projectMetrics} />

<style>
	.view-wrap {
		display: flex;
		position: relative;
		width: 100%;
	}
</style>
