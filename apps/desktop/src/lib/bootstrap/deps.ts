import { ActionService, ACTION_SERVICE } from '$lib/actions/actionService.svelte';
import {
	PromptService as AIPromptService,
	PROMPT_SERVICE as AI_PROMPT_SERVICE
} from '$lib/ai/promptService';
import { AIService, AI_SERVICE } from '$lib/ai/service';
import { EVENT_CONTEXT, EventContext } from '$lib/analytics/eventContext';
import { POSTHOG_WRAPPER, PostHogWrapper } from '$lib/analytics/posthog';
import { type IBackend } from '$lib/backend';
import { BACKEND } from '$lib/backend';
import ClipboardService, { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
import BaseBranchService, { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
import { BranchService, BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
import CLIManager, { CLI_MANAGER } from '$lib/cli/cli';
import { CLAUDE_CODE_SERVICE, ClaudeCodeService } from '$lib/codegen/claude';
import { AppSettings, APP_SETTINGS } from '$lib/config/appSettings';
import { SETTINGS_SERVICE, SettingsService } from '$lib/config/appSettingsV2';
import { GIT_CONFIG_SERVICE, GitConfigService } from '$lib/config/gitConfigService';
import DependencyService, { DEPENDENCY_SERVICE } from '$lib/dependencies/dependencyService.svelte';
import { DragStateService, DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
import { DropzoneRegistry, DROPZONE_REGISTRY } from '$lib/dragging/registry';
import {
	REORDER_DROPZONE_FACTORY,
	ReorderDropzoneFactory
} from '$lib/dragging/stackingReorderDropzoneManager';
import FeedFactory, { FEED_FACTORY } from '$lib/feed/feed';
import { FILE_SERVICE, FileService } from '$lib/files/fileService';
import { DefaultForgeFactory, DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
import { GITHUB_CLIENT, GitHubClient } from '$lib/forge/github/githubClient';
import { GitHubUserService, GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
import { GITLAB_CLIENT, GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import { GitService, GIT_SERVICE } from '$lib/git/gitService';
import { HISTORY_SERVICE, HistoryService } from '$lib/history/history';
import { OplogService, OPLOG_SERVICE } from '$lib/history/oplogService.svelte';
import { HOOKS_SERVICE, HooksService } from '$lib/hooks/hooksService';
import { DiffService, DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
import { IrcClient, IRC_CLIENT } from '$lib/irc/ircClient.svelte';
import { IrcService, IRC_SERVICE } from '$lib/irc/ircService.svelte';
import { ModeService, MODE_SERVICE } from '$lib/mode/modeService';
import { ProjectsService, PROJECTS_SERVICE } from '$lib/project/projectsService';
import { PROMPT_SERVICE, PromptService } from '$lib/prompt/promptService';
import { REMOTES_SERVICE, RemotesService } from '$lib/remotes/remotesService';
import RulesService, { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
import { RustSecretService, SECRET_SERVICE } from '$lib/secrets/secretsService';
import {
	FileSelectionManager,
	FILE_SELECTION_MANAGER
} from '$lib/selection/fileSelectionManager.svelte';
import { UncommittedService, UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
import { loadUserSettings, SETTINGS } from '$lib/settings/userSettings';
import { ShortcutService, SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
import { CodegenAnalytics, CODEGEN_ANALYTICS } from '$lib/soup/codegenAnalytics';
import { CommitAnalytics, COMMIT_ANALYTICS } from '$lib/soup/commitAnalytics';
import { StackService, STACK_SERVICE } from '$lib/stacks/stackService.svelte';
import { ClientState, CLIENT_STATE } from '$lib/state/clientState.svelte';
import { UiState, UI_STATE } from '$lib/state/uiState.svelte';
import { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import DataSharingService, { DATA_SHARING_SERVICE } from '$lib/support/dataSharing';
import { UPDATER_SERVICE, UpdaterService } from '$lib/updater/updater';
import {
	UpstreamIntegrationService,
	UPSTREAM_INTEGRATION_SERVICE
} from '$lib/upstream/upstreamIntegrationService.svelte';
import { USER } from '$lib/user/user';
import { USER_SERVICE, UserService } from '$lib/user/userService';
import { ResizeSync, RESIZE_SYNC } from '$lib/utils/resizeSync';
import URLService, { URL_SERVICE } from '$lib/utils/url';
import { WorktreeService, WORKTREE_SERVICE } from '$lib/worktree/worktreeService.svelte';
import { provideAll } from '@gitbutler/core/context';
import { FeedService, FEED_SERVICE } from '@gitbutler/shared/feeds/service';
import { HttpClient, HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
import {
	OrganizationService,
	ORGANIZATION_SERVICE
} from '@gitbutler/shared/organizations/organizationService';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { AppState, APP_STATE, APP_DISPATCH } from '@gitbutler/shared/redux/store.svelte';
import { UPLOADS_SERVICE, UploadsService } from '@gitbutler/shared/uploads/uploadsService';
import {
	UserService as CloudUserService,
	USER_SERVICE as CLOUD_USER_SERVICE
} from '@gitbutler/shared/users/userService';
import { FOCUS_MANAGER, FocusManager } from '@gitbutler/ui/focus/focusManager';
import {
	EXTERNAL_LINK_SERVICE,
	type ExternalLinkService
} from '@gitbutler/ui/utils/externalLinkService';
import { IMECompositionHandler, IME_COMPOSITION_HANDLER } from '@gitbutler/ui/utils/imeHandling';
import { PUBLIC_API_BASE_URL } from '$env/static/public';

export function initDependencies(args: {
	backend: IBackend;
	appSettings: AppSettings;
	settingsService: SettingsService;
	posthog: PostHogWrapper;
	eventContext: EventContext;
	homeDir: string;
}) {
	const { backend, settingsService, appSettings, homeDir, posthog, eventContext } = args;

	// ============================================================================
	// FOUNDATION LAYER - Core services that others depend on
	// ============================================================================

	const tokenMemoryService = new TokenMemoryService();
	const httpClient = new HttpClient(window.fetch, PUBLIC_API_BASE_URL, tokenMemoryService.token);
	const userSettings = loadUserSettings();
	const appState = new AppState();

	// ============================================================================
	// ANALYTICS & TELEMETRY
	// ============================================================================

	// ============================================================================
	// AUTHENTICATION & SECURITY
	// ============================================================================

	const secretsService = new RustSecretService(backend);
	const userService = new UserService(backend, httpClient, tokenMemoryService, posthog);

	// ============================================================================
	// FORGE CLIENTS & INTEGRATIONS
	// ============================================================================

	const gitHubClient = new GitHubClient();
	const gitLabClient = new GitLabClient();

	// ============================================================================
	// EXPERIMENTAL STUFF
	// ============================================================================

	// It's a bit weird that this is a dependency of `ClientState` while `IrcService`
	// depends on it.
	const ircClient = new IrcClient();

	// ============================================================================
	// STATE MANAGEMENT
	// ============================================================================

	const clientState = new ClientState(backend, gitHubClient, gitLabClient, ircClient, posthog);
	const githubUserService = new GitHubUserService(backend, clientState['githubApi']);

	const uiState = new UiState(
		reactive(() => clientState.uiState),
		clientState.dispatch
	);
	const ircService = new IrcService(clientState, clientState.dispatch, ircClient);

	// ============================================================================
	// CONFIGURATION & SETTINGS
	// ============================================================================

	const projectsService = new ProjectsService(clientState, homeDir, backend);
	const gitConfig = new GitConfigService(clientState, backend);

	// ============================================================================
	// AI SERVICES
	// ============================================================================

	const aiPromptService = new AIPromptService();
	const aiService = new AIService(gitConfig, secretsService, httpClient, tokenMemoryService);
	const claudeCodeService = new ClaudeCodeService(clientState['backendApi']);

	// ============================================================================
	// FORGE FACTORY
	// ============================================================================

	const forgeFactory = new DefaultForgeFactory({
		gitHubClient,
		gitLabClient,
		gitHubApi: clientState['githubApi'],
		gitLabApi: clientState['gitlabApi'],
		dispatch: clientState.dispatch,
		posthog: posthog
	});

	// ============================================================================
	// GIT & VERSION CONTROL
	// ============================================================================

	const gitService = new GitService(backend, clientState.backendApi);
	const baseBranchService = new BaseBranchService(clientState.backendApi);
	const branchService = new BranchService(clientState['backendApi']);
	const remotesService = new RemotesService(backend);
	const hooksService = new HooksService(backend);

	// ============================================================================
	// STACKS & WORKSPACE MANAGEMENT
	// ============================================================================

	const stackService = new StackService(
		clientState['backendApi'],
		clientState.dispatch,
		forgeFactory,
		uiState
	);
	const modeService = new ModeService(clientState['backendApi']);
	const rulesService = new RulesService(clientState['backendApi']);
	const worktreeService = new WorktreeService(clientState);

	// ============================================================================
	// FILE & DIFF MANAGEMENT
	// ============================================================================

	const fileService = new FileService(backend);
	const diffService = new DiffService(clientState);

	// ============================================================================
	// HISTORY & OPERATIONS
	// ============================================================================

	const historyService = new HistoryService(backend, clientState['backendApi']);
	const oplogService = new OplogService(clientState['backendApi']);
	const commitAnalytics = new CommitAnalytics(stackService, uiState, worktreeService, rulesService);
	const codegenAnalytics = new CodegenAnalytics(claudeCodeService, settingsService);

	// ============================================================================
	// SELECTION & EDITING
	// ============================================================================

	const uncommittedService = new UncommittedService(clientState, worktreeService, diffService);
	const fileSelectionManager = new FileSelectionManager(
		stackService,
		uncommittedService,
		worktreeService,
		oplogService,
		historyService
	);

	// ============================================================================
	// PROJECT & DEPENDENCY MANAGEMENT
	// ============================================================================

	const dependencyService = new DependencyService(worktreeService);

	// ============================================================================
	// ACTIONS & WORKFLOWS
	// ============================================================================

	const actionService = new ActionService(clientState['backendApi']);
	const upstreamIntegrationService = new UpstreamIntegrationService(
		clientState,
		stackService,
		projectsService
	);

	// ============================================================================
	// FEEDS & NOTIFICATIONS
	// ============================================================================

	const feedFactory = new FeedFactory(backend, stackService);
	const feedService = new FeedService(httpClient, appState.appDispatch);

	// ============================================================================
	// CLOUD SERVICES
	// ============================================================================

	const uploadsService = new UploadsService(httpClient);
	const organizationService = new OrganizationService(httpClient, appState.appDispatch);
	const cloudUserService = new CloudUserService(httpClient, appState.appDispatch);

	// ============================================================================
	// UI & INTERACTION
	// ============================================================================

	const focusManager = new FocusManager();
	const imeHandler = new IMECompositionHandler();
	const reorderDropzoneFactory = new ReorderDropzoneFactory(stackService);
	const shortcutService = new ShortcutService(backend);
	const dragStateService = new DragStateService();
	const dropzoneRegistry = new DropzoneRegistry();
	const resizeSync = new ResizeSync();

	// ============================================================================
	// SYSTEM SERVICES
	// ============================================================================

	const cliManager = new CLIManager(clientState['backendApi']);
	const dataSharingService = new DataSharingService(clientState['backendApi']);
	const promptService = new PromptService(backend);
	const updaterService = new UpdaterService(backend, posthog, shortcutService);

	// ============================================================================
	// UTILITIES
	// ============================================================================

	const urlService = new URLService(backend);
	const clipboardService = new ClipboardService(backend);
	const externalLinkService = {
		open: async (url) => await urlService.openExternalUrl(url)
	} satisfies ExternalLinkService;

	// ============================================================================
	// DEPENDENCY INJECTION REGISTRATION
	// ============================================================================

	provideAll([
		[ACTION_SERVICE, actionService],
		[AI_PROMPT_SERVICE, aiPromptService],
		[AI_SERVICE, aiService],
		[APP_DISPATCH, appState.appDispatch],
		[APP_SETTINGS, appSettings],
		[APP_STATE, appState],
		[BACKEND, backend],
		[BASE_BRANCH_SERVICE, baseBranchService],
		[BRANCH_SERVICE, branchService],
		[CLAUDE_CODE_SERVICE, claudeCodeService],
		[CLIENT_STATE, clientState],
		[CLIPBOARD_SERVICE, clipboardService],
		[CLI_MANAGER, cliManager],
		[CLOUD_USER_SERVICE, cloudUserService],
		[COMMIT_ANALYTICS, commitAnalytics],
		[CODEGEN_ANALYTICS, codegenAnalytics],
		[DATA_SHARING_SERVICE, dataSharingService],
		[DEFAULT_FORGE_FACTORY, forgeFactory],
		[DEPENDENCY_SERVICE, dependencyService],
		[DIFF_SERVICE, diffService],
		[DRAG_STATE_SERVICE, dragStateService],
		[DROPZONE_REGISTRY, dropzoneRegistry],
		[EVENT_CONTEXT, eventContext],
		[FEED_FACTORY, feedFactory],
		[FEED_SERVICE, feedService],
		[FILE_SERVICE, fileService],
		[FOCUS_MANAGER, focusManager],
		[GITHUB_CLIENT, gitHubClient],
		[GITHUB_USER_SERVICE, githubUserService],
		[GITLAB_CLIENT, gitLabClient],
		[GIT_CONFIG_SERVICE, gitConfig],
		[GIT_SERVICE, gitService],
		[HISTORY_SERVICE, historyService],
		[HOOKS_SERVICE, hooksService],
		[HTTP_CLIENT, httpClient],
		[FILE_SELECTION_MANAGER, fileSelectionManager],
		[IME_COMPOSITION_HANDLER, imeHandler],
		[IRC_CLIENT, ircClient],
		[IRC_SERVICE, ircService],
		[MODE_SERVICE, modeService],
		[OPLOG_SERVICE, oplogService],
		[ORGANIZATION_SERVICE, organizationService],
		[POSTHOG_WRAPPER, posthog],
		[PROJECTS_SERVICE, projectsService],
		[PROMPT_SERVICE, promptService],
		[REMOTES_SERVICE, remotesService],
		[RESIZE_SYNC, resizeSync],
		[RULES_SERVICE, rulesService],
		[SECRET_SERVICE, secretsService],
		[SETTINGS, userSettings],
		[SETTINGS_SERVICE, settingsService],
		[SHORTCUT_SERVICE, shortcutService],
		[STACK_SERVICE, stackService],
		[REORDER_DROPZONE_FACTORY, reorderDropzoneFactory],
		[UI_STATE, uiState],
		[UNCOMMITTED_SERVICE, uncommittedService],
		[UPDATER_SERVICE, updaterService],
		[UPLOADS_SERVICE, uploadsService],
		[UPSTREAM_INTEGRATION_SERVICE, upstreamIntegrationService],
		[URL_SERVICE, urlService],
		[USER, userService.user],
		[USER_SERVICE, userService],
		[WORKTREE_SERVICE, worktreeService],
		[EXTERNAL_LINK_SERVICE, externalLinkService]
	]);
}
