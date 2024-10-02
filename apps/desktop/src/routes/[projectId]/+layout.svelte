<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import FileMenuAction from '$lib/barmenuActions/FileMenuAction.svelte';
	import ProjectSettingsMenuAction from '$lib/barmenuActions/ProjectSettingsMenuAction.svelte';
	import { BaseBranch, NoDefaultTarget } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { BranchListingService, CombinedBranchListingService } from '$lib/branches/branchListing';
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import { CommitDragActionsFactory } from '$lib/commits/dragActions';
	import { CommitService } from '$lib/commits/service';
	import NoBaseBranch from '$lib/components/NoBaseBranch.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import { featureTopics } from '$lib/config/uiFeatureFlags';
	import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
	import { DefaultGitHostFactory } from '$lib/gitHost/gitHostFactory';
	import { octokitFromAccessToken } from '$lib/gitHost/github/octokit';
	import { createGitHostStore } from '$lib/gitHost/interface/gitHost';
	import { createGitHostListingServiceStore } from '$lib/gitHost/interface/gitHostListingService';
	import History from '$lib/history/History.svelte';
	import { HistoryService } from '$lib/history/history';
	import MetricsReporter from '$lib/metrics/MetricsReporter.svelte';
	import { ModeService } from '$lib/modes/service';
	import Navigation from '$lib/navigation/Navigation.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import { RemoteBranchService } from '$lib/stores/remoteBranches';
	import CreateIssueModal from '$lib/topics/CreateIssueModal.svelte';
	import CreateTopicModal from '$lib/topics/CreateTopicModal.svelte';
	import { TopicService } from '$lib/topics/service';
	import { UncommitedFilesWatcher } from '$lib/uncommitedFiles/watcher';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { debounce } from '$lib/utils/debounce';
	import { BranchController } from '$lib/vbranches/branchController';
	import { UpstreamIntegrationService } from '$lib/vbranches/upstreamIntegrationService';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { onDestroy, setContext, type Snippet } from 'svelte';
	import { derived as storeDerived } from 'svelte/store';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const {
		vbranchService,
		project,
		projectId,
		projectService,
		projectMetrics,
		baseBranchService,
		remoteBranchService,
		modeService,
		userService,
		fetchSignal
	} = $derived(data);

	const branchesError = $derived(vbranchService.branchesError);
	const baseBranch = $derived(baseBranchService.base);
	const remoteUrl = $derived($baseBranch?.remoteUrl);
	const forkUrl = $derived($baseBranch?.pushRemoteUrl);
	const user = $derived(userService.user);
	const accessToken = $derived($user?.github_access_token);
	const baseError = $derived(baseBranchService.error);
	const projectError = $derived(projectService.error);

	$effect.pre(() => {
		setContext(HistoryService, data.historyService);
		setContext(VirtualBranchService, data.vbranchService);
		setContext(BranchController, data.branchController);
		setContext(BaseBranchService, data.baseBranchService);
		setContext(CommitService, data.commitService);
		setContext(BaseBranch, baseBranch);
		setContext(Project, project);
		setContext(BranchDragActionsFactory, data.branchDragActionsFactory);
		setContext(CommitDragActionsFactory, data.commitDragActionsFactory);
		setContext(ReorderDropzoneManagerFactory, data.reorderDropzoneManagerFactory);
		setContext(RemoteBranchService, data.remoteBranchService);
		setContext(BranchListingService, data.branchListingService);
		setContext(ModeService, data.modeService);
		setContext(UncommitedFilesWatcher, data.uncommitedFileWatcher);
		setContext(UpstreamIntegrationService, data.upstreamIntegrationService);
	});

	let intervalId: any;

	const showHistoryView = persisted(false, 'showHistoryView');
	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);
	const gitHostFactory = $derived(new DefaultGitHostFactory(octokit));
	const repoInfo = $derived(remoteUrl ? parseRemoteUrl(remoteUrl) : undefined);
	const forkInfo = $derived(forkUrl && forkUrl !== remoteUrl ? parseRemoteUrl(forkUrl) : undefined);
	const baseBranchName = $derived($baseBranch?.shortName);

	const listServiceStore = createGitHostListingServiceStore(undefined);
	const gitHostStore = createGitHostStore(undefined);
	const gitHostIssueSerice = storeDerived(gitHostStore, (gitHostStore) =>
		gitHostStore?.issueService()
	);

	$effect.pre(() => {
		const topicService = new TopicService(project, gitHostIssueSerice);
		setContext(TopicService, topicService);
	});

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
		async () => await remoteBranchService.refresh(),
		500
	);
	$effect(() => {
		if ($baseBranch || $head || $fetch) debouncedRemoteBranchRefresh();
	});

	$effect(() => {
		const gitHost =
			repoInfo && baseBranchName
				? gitHostFactory.build(repoInfo, baseBranchName, forkInfo)
				: undefined;

		const ghListService = gitHost?.listService();

		listServiceStore.set(ghListService);
		gitHostStore.set(gitHost);
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

	onDestroy(() => {
		clearFetchInterval();
	});

	const topicsEnabled = featureTopics();
</script>

{#if $topicsEnabled}
	{#if $gitHostStore?.issueService()}
		<CreateIssueModal registerKeypress />
	{/if}
	<CreateTopicModal registerKeypress />
{/if}

<!-- forces components to be recreated when projectId changes -->
{#key projectId}
	<ProjectSettingsMenuAction
		showHistory={$showHistoryView}
		onHistoryShow={(show) => ($showHistoryView = show)}
	/>
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
					<History on:hide={() => ($showHistoryView = false)} />
				{/if}
				{@render children()}
			</div>
		{:else if $mode?.type === 'OutsideWorkspace'}
			<NotOnGitButlerBranch baseBranch={$baseBranch} />
		{/if}
	{/if}
	<MetricsReporter {projectMetrics} />
{/key}

<style>
	.view-wrap {
		position: relative;
		display: flex;
		width: 100%;
	}
</style>
