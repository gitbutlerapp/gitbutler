<script lang="ts">
	import '@gitbutler/ui/main.css';
	import '../styles/styles.css';

	import { browser, dev } from '$app/environment';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import AppUpdater from '$components/AppUpdater.svelte';
	import GlobalModal from '$components/GlobalModal.svelte';
	import GlobalSettingsMenuAction from '$components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$components/PromptModal.svelte';
	import ReloadMenuAction from '$components/ReloadMenuAction.svelte';
	import ReloadWarning from '$components/ReloadWarning.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import SwitchThemeMenuAction from '$components/SwitchThemeMenuAction.svelte';
	import ToastController from '$components/ToastController.svelte';
	import ZoomInOutMenuAction from '$components/ZoomInOutMenuAction.svelte';
	import { ActionService, ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { PROMPT_SERVICE as AI_PROMPT_SERVICE } from '$lib/ai/promptService';
	import { AI_SERVICE, AIService } from '$lib/ai/service';
	import { EVENT_CONTEXT } from '$lib/analytics/eventContext';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { invoke } from '$lib/backend/ipc';
	import BaseBranchService, { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchService, BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { CommitService, COMMIT_SERVICE } from '$lib/commits/commitService.svelte';
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { ircEnabled, ircServer } from '$lib/config/uiFeatureFlags';
	import DependencyService, {
		DEPENDENCY_SERVICE
	} from '$lib/dependencies/dependencyService.svelte';
	import { DragStateService, DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
	import { DropzoneRegistry, DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import FeedFactory, { FEED_FACTORY } from '$lib/feed/feed';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { DefaultForgeFactory, DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { GitHubClient, GITHUB_CLIENT } from '$lib/forge/github/githubClient';
	import {
		GitHubUserService,
		GITHUB_USER_SERVICE
	} from '$lib/forge/github/githubUserService.svelte';
	import { GitLabClient, GITLAB_CLIENT } from '$lib/forge/gitlab/gitlabClient.svelte';
	import { GitService, GIT_SERVICE } from '$lib/git/gitService';
	import { HISTORY_SERVICE, HistoryService } from '$lib/history/history';
	import { OplogService, OPLOG_SERVICE } from '$lib/history/oplogService.svelte';
	import { HOOKS_SERVICE } from '$lib/hooks/hooksService';
	import { DiffService, DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import {
		IntelligentScrollingService,
		INTELLIGENT_SCROLLING_SERVICE
	} from '$lib/intelligentScrolling/service';
	import { IrcClient, IRC_CLIENT } from '$lib/irc/ircClient.svelte';
	import { IrcService, IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { ModeService, MODE_SERVICE } from '$lib/mode/modeService';
	import { ProjectsService, PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { PROMPT_SERVICE } from '$lib/prompt/promptService';
	import { REMOTES_SERVICE } from '$lib/remotes/remotesService';
	import RulesService, { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { RustSecretService, SECRET_SERVICE } from '$lib/secrets/secretsService';
	import { IdSelection, ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import {
		UncommittedService,
		UNCOMMITTED_SERVICE
	} from '$lib/selection/uncommittedService.svelte';
	import { loadUserSettings, SETTINGS } from '$lib/settings/userSettings';
	import { ShortcutService, SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { CommitAnalytics, COMMIT_ANALYTICS } from '$lib/soup/commitAnalytics';
	import { StackService, STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { ClientState, CLIENT_STATE } from '$lib/state/clientState.svelte';
	import { UiState, UI_STATE } from '$lib/state/uiState.svelte';
	import { UPDATER_SERVICE, UpdaterService } from '$lib/updater/updater';
	import {
		UpstreamIntegrationService,
		UPSTREAM_INTEGRATION_SERVICE
	} from '$lib/upstream/upstreamIntegrationService.svelte';
	import { USER } from '$lib/user/user';
	import { USER_SERVICE } from '$lib/user/userService';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { ResizeSync, RESIZE_SYNC } from '$lib/utils/resizeSync';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { openExternalUrl } from '$lib/utils/url';
	import { WorktreeService, WORKTREE_SERVICE } from '$lib/worktree/worktreeService.svelte';
	import { provide } from '@gitbutler/shared/context';
	import { FeedService, FEED_SERVICE } from '@gitbutler/shared/feeds/service';
	import { HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
	import {
		OrganizationService,
		ORGANIZATION_SERVICE
	} from '@gitbutler/shared/organizations/organizationService';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { AppState, APP_STATE, APP_DISPATCH } from '@gitbutler/shared/redux/store.svelte';
	import { UPLOADS_SERVICE } from '@gitbutler/shared/uploads/uploadsService';
	import {
		UserService as CloudUserService,
		USER_SERVICE as CLOUD_USER_SERVICE
	} from '@gitbutler/shared/users/userService';
	import { ChipToastContainer } from '@gitbutler/ui';
	import { setExternalLinkService } from '@gitbutler/ui/utils/externalLinkService';
	import { onMount, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const userSettings = loadUserSettings();
	provide(SETTINGS, userSettings);

	const appState = new AppState();

	const gitHubClient = new GitHubClient();
	const gitLabClient = new GitLabClient();
	provide(GITHUB_CLIENT, gitHubClient);
	provide(GITLAB_CLIENT, gitLabClient);
	const user = data.userService.user;
	const accessToken = $derived($user?.github_access_token);
	$effect(() => gitHubClient.setToken(accessToken));

	const ircClient = new IrcClient();
	provide(IRC_CLIENT, ircClient);

	const clientState = new ClientState(
		data.tauri,
		gitHubClient,
		gitLabClient,
		ircClient,
		data.posthog
	);

	const ircService = new IrcService(clientState, clientState.dispatch, ircClient);
	provide(IRC_SERVICE, ircService);

	$effect(() => {
		if (!$ircEnabled || !$ircServer || !$user || !$user.login) {
			return;
		}
		ircClient.connect({ server: $ircServer, nick: $user.login });
		return () => {
			ircService.disconnect();
		};
	});

	const forgeFactory = new DefaultForgeFactory({
		gitHubClient,
		gitLabClient,
		gitHubApi: clientState['githubApi'],
		gitLabApi: clientState['gitlabApi'],
		dispatch: clientState.dispatch,
		posthog: data.posthog
	});

	const uiStateSlice = $derived(clientState.uiState);
	const uiState = new UiState(
		reactive(() => uiStateSlice),
		clientState.dispatch
	);
	provide(UI_STATE, uiState);
	const intelligentScrollingService = new IntelligentScrollingService(uiState);
	provide(INTELLIGENT_SCROLLING_SERVICE, intelligentScrollingService);

	const stackService = new StackService(
		clientState['backendApi'],
		clientState.dispatch,
		forgeFactory,
		uiState
	);
	const feedFactory = new FeedFactory(data.tauri, stackService);
	const rulesService = new RulesService(clientState['backendApi']);
	const modeService = $derived(new ModeService(clientState['backendApi']));
	$effect.pre(() => {
		provide(MODE_SERVICE, modeService);
	});
	const actionService = new ActionService(clientState['backendApi']);
	const oplogService = new OplogService(clientState['backendApi']);
	const baseBranchService = new BaseBranchService(clientState.backendApi);
	const worktreeService = new WorktreeService(clientState);
	const feedService = new FeedService(data.httpClient, appState.appDispatch);
	const organizationService = new OrganizationService(data.httpClient, appState.appDispatch);
	const cloudUserService = new CloudUserService(data.httpClient, appState.appDispatch);
	const dependecyService = new DependencyService(worktreeService);
	const diffService = new DiffService(clientState);

	const uncommittedService = new UncommittedService(clientState, worktreeService, diffService);
	provide(UNCOMMITTED_SERVICE, uncommittedService);
	const historyService = new HistoryService(clientState['backendApi']);
	provide(HISTORY_SERVICE, historyService);

	const idSelection = new IdSelection(
		stackService,
		uncommittedService,
		worktreeService,
		oplogService,
		historyService
	);

	const projectsService = new ProjectsService(clientState, data.homeDir, data.httpClient);
	provide(PROJECTS_SERVICE, projectsService);

	const shortcutService = new ShortcutService(data.tauri);
	const updaterService = new UpdaterService(data.tauri, data.posthog, shortcutService);
	const commitService = new CommitService();

	const upstreamIntegrationService = new UpstreamIntegrationService(
		clientState,
		stackService,
		projectsService
	);

	const commitAnalytics = new CommitAnalytics(stackService, uiState, worktreeService);
	provide(COMMIT_ANALYTICS, commitAnalytics);

	const branchService = new BranchService(clientState['backendApi']);
	provide(BRANCH_SERVICE, branchService);

	clientState.initPersist();

	provide(DEFAULT_FORGE_FACTORY, forgeFactory);

	$effect(() => shortcutService.listen());

	setExternalLinkService({ open: openExternalUrl });

	const secretsService = new RustSecretService(data.gitConfig);
	provide(SECRET_SERVICE, secretsService);

	const aiService = new AIService(
		data.gitConfig,
		secretsService,
		data.httpClient,
		data.tokenMemoryService
	);
	provide(AI_SERVICE, aiService);

	provide(APP_STATE, appState);
	provide(APP_DISPATCH, appState.appDispatch);
	provide(CLIENT_STATE, clientState);
	provide(FEED_SERVICE, feedService);
	provide(ORGANIZATION_SERVICE, organizationService);
	provide(CLOUD_USER_SERVICE, cloudUserService);
	provide(HOOKS_SERVICE, data.hooksService);
	provide(SETTINGS_SERVICE, data.settingsService);
	provide(FILE_SERVICE, data.fileService);
	provide(COMMIT_SERVICE, commitService);

	// Setters do not need to be reactive since `data` never updates
	provide(POSTHOG_WRAPPER, data.posthog);
	provide(USER_SERVICE, data.userService);
	provide(UPDATER_SERVICE, updaterService);
	provide(GIT_CONFIG_SERVICE, data.gitConfig);
	provide(PROMPT_SERVICE, data.promptService);
	provide(HTTP_CLIENT, data.httpClient);
	provide(USER, data.userService.user);
	provide(REMOTES_SERVICE, data.remotesService);
	provide(AI_PROMPT_SERVICE, data.aiPromptService);
	provide(APP_SETTINGS, data.appSettings);
	provide(EVENT_CONTEXT, data.eventContext);
	provide(STACK_SERVICE, stackService);
	provide(FEED_FACTORY, feedFactory);
	provide(RULES_SERVICE, rulesService);
	provide(ACTION_SERVICE, actionService);
	provide(OPLOG_SERVICE, oplogService);
	provide(BASE_BRANCH_SERVICE, baseBranchService);
	provide(UPSTREAM_INTEGRATION_SERVICE, upstreamIntegrationService);
	provide(WORKTREE_SERVICE, worktreeService);
	provide(SHORTCUT_SERVICE, shortcutService);
	provide(DIFF_SERVICE, diffService);
	provide(UPLOADS_SERVICE, data.uploadsService);
	provide(DEPENDENCY_SERVICE, dependecyService);
	provide(ID_SELECTION, idSelection);
	provide(DROPZONE_REGISTRY, new DropzoneRegistry());
	provide(DRAG_STATE_SERVICE, new DragStateService());
	provide(RESIZE_SYNC, new ResizeSync());
	provide(GIT_SERVICE, new GitService(data.tauri));

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
	provide(GITHUB_USER_SERVICE, githubUserService);

	let shareIssueModal: ShareIssueModal;

	const projectId = $derived(page.params.projectId);
	$effect(() => {
		if (projectId) {
			projectsService.setLastOpenedProject(projectId);
		}
	});

	onMount(() => {
		return unsubscribe(
			events.on('goto', async (path: string) => await goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show())
		);
	});

	const handleKeyDown = createKeybind({
		// Toggle v3 workspace APIs on/off
		'w s 3': () => {
			settingsService.updateFeatureFlags({ ws3: !$settingsStore?.featureFlags.ws3 });
		},
		// This is a debug tool to see how the commit-graph looks like, the basis for all workspace computation.
		// For good measure, it also shows the workspace.
		'd o t': async () => {
			const projectId = page.params.projectId;
			await invoke('show_graph_svg', { projectId });
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

	/** These are made available on the window object for easier debugging. */
	(window as any)['uiState'] = uiState;
	(window as any)['idSelection'] = idSelection;
	(window as any)['clientState'] = clientState;
</script>

<svelte:window
	ondrop={(e) => e.preventDefault()}
	ondragover={(e) => e.preventDefault()}
	onkeydown={handleKeyDown}
/>

<div class="app-root" role="application" oncontextmenu={(e) => !dev && e.preventDefault()}>
	{@render children()}
</div>
<Toaster />
<ShareIssueModal bind:this={shareIssueModal} />
<ToastController />
<ChipToastContainer />
<AppUpdater />
<PromptModal />
<ZoomInOutMenuAction />
<GlobalSettingsMenuAction />
<ReloadMenuAction />
<SwitchThemeMenuAction />
<GlobalModal />

{#if import.meta.env.MODE === 'development'}
	<ReloadWarning />
{/if}

<style lang="postcss">
	.app-root {
		display: flex;
		height: 100%;
		cursor: default;
	}
</style>
