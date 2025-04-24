<script lang="ts">
	import '@gitbutler/ui/main.css';
	import '../styles/styles.css';

	import AppUpdater from '$components/AppUpdater.svelte';
	import GlobalSettingsMenuAction from '$components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$components/PromptModal.svelte';
	import ReloadMenuAction from '$components/ReloadMenuAction.svelte';
	import ReloadWarning from '$components/ReloadWarning.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import SwitchThemeMenuAction from '$components/SwitchThemeMenuAction.svelte';
	import ToastController from '$components/ToastController.svelte';
	import ZoomInOutMenuAction from '$components/ZoomInOutMenuAction.svelte';
	import LineSelection from '$components/v3/unifiedDiffLineSelection.svelte';
	import { PromptService as AIPromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { CommandService, invoke } from '$lib/backend/ipc';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import {
		IpcNameNormalizationService,
		setNameNormalizationServiceContext
	} from '$lib/branches/nameNormalizationService';
	import { CommitService } from '$lib/commits/commitService.svelte';
	import { AppSettings } from '$lib/config/appSettings';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { ircEnabled, ircServer } from '$lib/config/uiFeatureFlags';
	import DependencyService from '$lib/dependencies/dependencyService.svelte';
	import { FileService } from '$lib/files/fileService';
	import { ButRequestDetailsService } from '$lib/forge/butRequestDetailsService';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { GitHubClient } from '$lib/forge/github/githubClient';
	import { GitHubUserService } from '$lib/forge/github/githubUserService.svelte';
	import { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
	import { HooksService } from '$lib/hooks/hooksService';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { IrcClient } from '$lib/irc/ircClient.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { platformName } from '$lib/platform/platform';
	import { ProjectsService } from '$lib/project/projectsService';
	import { PromptService } from '$lib/prompt/promptService';
	import { RemotesService } from '$lib/remotes/remotesService';
	import { setSecretsService } from '$lib/secrets/secretsService';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { SETTINGS, loadUserSettings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { ClientState } from '$lib/state/clientState.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { UpdaterService } from '$lib/updater/updater';
	import { UpstreamIntegrationService } from '$lib/upstream/upstreamIntegrationService.svelte';
	import { User } from '$lib/user/user';
	import { UserService } from '$lib/user/userService';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { openExternalUrl } from '$lib/utils/url';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { PatchCommitService as CloudPatchCommitService } from '@gitbutler/shared/patches/patchCommitService';
	import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { reactive } from '@gitbutler/shared/storeUtils';
	import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
	import { UserService as CloudUserService } from '@gitbutler/shared/users/userService';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import { LineManagerFactory as StackingLineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import { setExternalLinkService } from '@gitbutler/ui/link/externalLinkService';
	import { onMount, setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { dev } from '$app/environment';
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { beforeNavigate, afterNavigate } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const userSettings = loadUserSettings();
	setContext(SETTINGS, userSettings);

	const appState = new AppState();

	const gitHubClient = new GitHubClient();
	const gitLabClient = new GitLabClient();
	setContext(GitHubClient, gitHubClient);
	setContext(GitLabClient, gitLabClient);
	const user = data.userService.user;
	const accessToken = $derived($user?.github_access_token);
	$effect(() => gitHubClient.setToken(accessToken));

	const ircClient = new IrcClient();
	setContext(IrcClient, ircClient);

	const clientState = new ClientState(
		data.tauri,
		gitHubClient,
		gitLabClient,
		ircClient,
		data.posthog
	);

	const ircService = new IrcService(clientState, clientState.dispatch, ircClient);
	setContext(IrcService, ircService);

	$effect(() => {
		if (!$ircEnabled || !$ircServer || !$user || !$user.login) {
			return;
		}
		ircClient.connect({ server: $ircServer, nick: $user.login });
		return () => {
			ircService.disconnect();
		};
	});

	const changeSelection = $derived(clientState.changeSelection);
	const changeSelectionService = new ChangeSelectionService(
		reactive(() => changeSelection),
		clientState.dispatch
	);
	const unifiedDiffLineSelection = new LineSelection(changeSelectionService);

	const forgeFactory = new DefaultForgeFactory({
		gitHubClient,
		gitLabClient,
		gitHubApi: clientState['githubApi'],
		gitLabApi: clientState['gitlabApi'],
		dispatch: clientState.dispatch,
		posthog: data.posthog,
		projectMetrics: data.projectMetrics
	});

	const uiStateSlice = $derived(clientState.uiState);
	const uiState = new UiState(
		reactive(() => uiStateSlice),
		clientState.dispatch
	);
	setContext(UiState, uiState);

	const stackService = new StackService(clientState['backendApi'], forgeFactory, uiState);
	const baseBranchService = new BaseBranchService(clientState.backendApi);
	const worktreeService = new WorktreeService(clientState);
	const feedService = new FeedService(data.cloud, appState.appDispatch);
	const organizationService = new OrganizationService(data.cloud, appState.appDispatch);
	const cloudUserService = new CloudUserService(data.cloud, appState.appDispatch);
	const cloudProjectService = new CloudProjectService(data.cloud, appState.appDispatch);
	const dependecyService = new DependencyService(clientState.backendApi);

	const cloudBranchService = new CloudBranchService(data.cloud, appState.appDispatch);
	const cloudPatchService = new CloudPatchCommitService(data.cloud, appState.appDispatch);
	const repositoryIdLookupService = new RepositoryIdLookupService(data.cloud, appState.appDispatch);
	const latestBranchLookupService = new LatestBranchLookupService(data.cloud, appState.appDispatch);
	const webRoutesService = new WebRoutesService(env.PUBLIC_CLOUD_BASE_URL);
	const diffService = new DiffService(clientState);
	const shortcutService = new ShortcutService(data.tauri);
	const commitService = new CommitService();
	const butRequestDetailsService = new ButRequestDetailsService(
		cloudBranchService,
		latestBranchLookupService
	);
	const upstreamIntegrationService = new UpstreamIntegrationService(
		clientState,
		stackService,
		data.projectsService,
		cloudProjectService,
		cloudBranchService,
		latestBranchLookupService
	);

	const branchService = new BranchService(clientState['backendApi']);
	setContext(BranchService, branchService);

	clientState.initPersist();

	setContext(DefaultForgeFactory, forgeFactory);

	shortcutService.listen();

	setExternalLinkService({ open: openExternalUrl });

	setContext(AppState, appState);
	setContext(AppDispatch, appState.appDispatch);
	setContext(ChangeSelectionService, changeSelectionService);
	setContext(LineSelection, unifiedDiffLineSelection);
	setContext(ClientState, clientState);
	setContext(FeedService, feedService);
	setContext(OrganizationService, organizationService);
	setContext(CloudUserService, cloudUserService);
	setContext(CloudProjectService, cloudProjectService);
	setContext(CloudBranchService, cloudBranchService);
	setContext(CloudPatchCommitService, cloudPatchService);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	setContext(LatestBranchLookupService, latestBranchLookupService);
	setContext(WebRoutesService, webRoutesService);
	setContext(HooksService, data.hooksService);
	setContext(SettingsService, data.settingsService);
	setContext(FileService, data.fileService);
	setContext(CommitService, commitService);
	setContext(ButRequestDetailsService, butRequestDetailsService);

	// Setters do not need to be reactive since `data` never updates
	setSecretsService(data.secretsService);
	setContext(PostHogWrapper, data.posthog);
	setContext(CommandService, data.commandService);
	setContext(UserService, data.userService);
	setContext(ProjectsService, data.projectsService);
	setContext(UpdaterService, data.updaterService);
	setContext(GitConfigService, data.gitConfig);
	setContext(AIService, data.aiService);
	setContext(PromptService, data.promptService);
	setContext(HttpClient, data.cloud);
	setContext(User, data.userService.user);
	setContext(RemotesService, data.remotesService);
	setContext(AIPromptService, data.aiPromptService);
	setContext(LineManagerFactory, data.lineManagerFactory);
	setContext(StackingLineManagerFactory, data.stackingLineManagerFactory);
	setContext(AppSettings, data.appSettings);
	setContext(StackService, stackService);
	setContext(BaseBranchService, baseBranchService);
	setContext(UpstreamIntegrationService, upstreamIntegrationService);
	setContext(WorktreeService, worktreeService);
	setContext(ShortcutService, shortcutService);
	setContext(DiffService, diffService);
	setContext(UploadsService, data.uploadsService);
	setContext(DependencyService, dependecyService);

	setNameNormalizationServiceContext(new IpcNameNormalizationService(invoke));

	const settingsService = data.settingsService;
	const settingsStore = settingsService.appSettings;

	// Special initialization to capture pageviews for single page apps.
	if (browser) {
		beforeNavigate(() => data.posthog.capture('$pageleave'));
		afterNavigate(() => data.posthog.capture('$pageview'));
	}

	// This store is literally only used once, on GitHub oauth, to set the
	// gh username on the user object. Furthermore, it isn't used anywhere.
	// TODO: Remove the gh username completely?
	const githubUserService = new GitHubUserService(data.tauri, clientState['githubApi']);
	setContext(GitHubUserService, githubUserService);

	let shareIssueModal: ShareIssueModal;

	onMount(() => {
		return unsubscribe(
			events.on('goto', async (path: string) => await goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show())
		);
	});

	const handleKeyDown = createKeybind({
		// Toggle v3 design on/off
		'v 3': () => {
			settingsService.updateFeatureFlags({ v3: !$settingsStore?.featureFlags.v3 });
		},
		// This is a debug tool to learn about environment variables actually present - only available if the backend is in debug mode.
		'e n v': async () => {
			let env = await invoke('env_vars');
			// eslint-disable-next-line no-console
			console.log(env);
			(window as any).tauriEnv = env;
			// eslint-disable-next-line no-console
			console.log('Also written to window.tauriEnv');
		}
	});
</script>

<svelte:window
	ondrop={(e) => e.preventDefault()}
	ondragover={(e) => e.preventDefault()}
	onkeydown={handleKeyDown}
/>

<div class="app-root" role="application" oncontextmenu={(e) => !dev && e.preventDefault()}>
	{#if platformName === 'macos' && !$settingsStore?.featureFlags.v3}
		<div class="drag-region" data-tauri-drag-region></div>
	{/if}
	{@render children()}
</div>
<Toaster />
<ShareIssueModal bind:this={shareIssueModal} />
<ToastController />
<AppUpdater />
<PromptModal />
<ZoomInOutMenuAction />
<GlobalSettingsMenuAction />
<ReloadMenuAction />
<SwitchThemeMenuAction />

{#if import.meta.env.MODE === 'development'}
	<ReloadWarning />
{/if}

<style lang="postcss">
	.app-root {
		display: flex;
		height: 100%;
		user-select: none;
		cursor: default;
	}

	.drag-region {
		z-index: var(--z-modal);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 14px;
	}
</style>
