import { ActionService, ACTION_SERVICE } from "$lib/actions/actionService.svelte";
import {
	PromptService as AIPromptService,
	PROMPT_SERVICE as AI_PROMPT_SERVICE,
} from "$lib/ai/aiPromptService";
import { AIService, AI_SERVICE } from "$lib/ai/service";
import { CommitAnalytics, COMMIT_ANALYTICS } from "$lib/analytics/commitAnalytics";
import { type IBackend } from "$lib/backend";
import { BACKEND } from "$lib/backend";
import ClipboardService, { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
import URLService, { URL_SERVICE } from "$lib/backend/url";
import BaseBranchService, { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { BranchService, BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
import { ATTACHMENT_SERVICE, AttachmentService } from "$lib/codegen/attachmentService.svelte";
import { CLAUDE_CODE_SERVICE, ClaudeCodeService } from "$lib/codegen/claude";
import { CodegenAnalytics, CODEGEN_ANALYTICS } from "$lib/codegen/codegenAnalytics";
import CLIManager, { CLI_MANAGER } from "$lib/config/cli";
import { GIT_CONFIG_SERVICE, GitConfigService } from "$lib/config/gitConfigService";
import DependencyService, { DEPENDENCY_SERVICE } from "$lib/dependencies/dependencyService.svelte";
import { DropzoneRegistry, DROPZONE_REGISTRY } from "$lib/dragging/registry";
import {
	REORDER_DROPZONE_FACTORY,
	ReorderDropzoneFactory,
} from "$lib/dragging/stackingReorderDropzoneManager";
import { FILE_SERVICE, FileService } from "$lib/files/fileService";
import { ResizeSync, RESIZE_SYNC } from "$lib/floating/resizeSync";
import { DefaultForgeFactory, DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
import { GITHUB_CLIENT, GitHubClient } from "$lib/forge/github/githubClient";
import { GitHubUserService, GITHUB_USER_SERVICE } from "$lib/forge/github/githubUserService.svelte";
import { GITLAB_CLIENT, GitLabClient } from "$lib/forge/gitlab/gitlabClient.svelte";
import { GITLAB_USER_SERVICE, GitLabUserService } from "$lib/forge/gitlab/gitlabUserService.svelte";
import { CherryApplyService, CHERRY_APPLY_SERVICE } from "$lib/git/cherryApplyService";
import { GitService, GIT_SERVICE } from "$lib/git/gitService";
import { HOOKS_SERVICE, HooksService } from "$lib/git/hooksService";
import { REMOTES_SERVICE, RemotesService } from "$lib/git/remotesService";
import { HISTORY_SERVICE, HistoryService } from "$lib/history/history";
import { OplogService, OPLOG_SERVICE } from "$lib/history/oplogService.svelte";
import { DiffService, DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
import { IrcApiService, IRC_API_SERVICE } from "$lib/irc/ircApiService";
import { IRC_SESSION_BRIDGE, IrcSessionBridge } from "$lib/irc/sessionBridge.svelte";
import {
	WORKING_FILES_BROADCAST,
	WorkingFilesBroadcast,
} from "$lib/irc/workingFilesBroadcast.svelte";
import { ModeService, MODE_SERVICE } from "$lib/mode/modeService";
import { ProjectsService, PROJECTS_SERVICE } from "$lib/project/projectsService";
import { PROMPT_SERVICE, PromptService } from "$lib/prompt/promptService";
import RulesService, { RULES_SERVICE } from "$lib/rules/rulesService.svelte";
import { RustSecretService, SECRET_SERVICE } from "$lib/secrets/secretsService";
import {
	FileSelectionManager,
	FILE_SELECTION_MANAGER,
} from "$lib/selection/fileSelectionManager.svelte";
import { UncommittedService, UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
import { SETTINGS_SERVICE, SettingsService } from "$lib/settings/appSettings";
import { loadUserSettings, SETTINGS } from "$lib/settings/userSettings";
import { ShortcutService, SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
import { StackService, STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { ClientState, CLIENT_STATE } from "$lib/state/clientState.svelte";
import { UiState, UI_STATE, uiStateSlice } from "$lib/state/uiState.svelte";
import DataSharingService, { DATA_SHARING_SERVICE } from "$lib/support/dataSharing";
import { EVENT_CONTEXT, EventContext } from "$lib/telemetry/eventContext";
import { POSTHOG_WRAPPER, PostHogWrapper } from "$lib/telemetry/posthog";
import { UPDATER_SERVICE, UpdaterService } from "$lib/updater/updater";
import {
	UpstreamIntegrationService,
	UPSTREAM_INTEGRATION_SERVICE,
} from "$lib/upstream/upstreamIntegrationService.svelte";
import { TokenMemoryService } from "$lib/user/tokenMemoryService";
import { USER } from "$lib/user/user";
import { USER_SERVICE, UserService } from "$lib/user/userService";
import { WorktreeService, WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
import { provideAll } from "@gitbutler/core/context";
import { FeedService, FEED_SERVICE } from "@gitbutler/shared/feeds/service";
import { HttpClient, HTTP_CLIENT } from "@gitbutler/shared/network/httpClient";
import {
	OrganizationService,
	ORGANIZATION_SERVICE,
} from "@gitbutler/shared/organizations/organizationService";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import { AppState, APP_STATE, APP_DISPATCH } from "@gitbutler/shared/redux/store.svelte";
import { UPLOADS_SERVICE, UploadsService } from "@gitbutler/shared/uploads/uploadsService";
import {
	UserService as CloudUserService,
	USER_SERVICE as CLOUD_USER_SERVICE,
} from "@gitbutler/shared/users/userService";
import { DragStateService, DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
import { FModeManager } from "@gitbutler/ui/focus/fModeManager";
import { FOCUS_MANAGER, FocusManager } from "@gitbutler/ui/focus/focusManager";
import {
	EXTERNAL_LINK_SERVICE,
	type ExternalLinkService,
} from "@gitbutler/ui/utils/externalLinkService";
import { IMECompositionHandler, IME_COMPOSITION_HANDLER } from "@gitbutler/ui/utils/imeHandling";
import type { Settings } from "@gitbutler/core/api";
import { PUBLIC_API_BASE_URL } from "$env/static/public";

export function initDependencies(args: {
	backend: IBackend;
	appSettings: Settings.AppSettings;
	settingsService: SettingsService;
	posthog: PostHogWrapper;
	eventContext: EventContext;
	homeDir: string;
}) {
	const { backend, settingsService, appSettings, homeDir, posthog, eventContext } = args;

	// ============================================================================
	// FOUNDATION LAYER - Core services that others depend on
	// ============================================================================

	const userSettings = loadUserSettings(backend.platformName);
	const appState = new AppState();

	// ============================================================================
	// ANALYTICS & TELEMETRY
	// ============================================================================

	// ============================================================================
	// AUTHENTICATION & SECURITY
	// ============================================================================

	const secretsService = new RustSecretService(backend);
	const tokenMemoryService = new TokenMemoryService();
	const httpClient = new HttpClient(window.fetch, PUBLIC_API_BASE_URL, tokenMemoryService.token);

	// ============================================================================
	// FORGE CLIENTS & INTEGRATIONS
	// ============================================================================

	const gitHubClient = new GitHubClient();
	const gitLabClient = new GitLabClient();

	// ============================================================================
	// STATE MANAGEMENT
	// ============================================================================

	const clientState = new ClientState(backend, gitHubClient, gitLabClient, posthog);
	const githubUserService = new GitHubUserService(clientState.backendApi);
	const gitlabUserService = new GitLabUserService(clientState.backendApi, secretsService);

	const uiState = new UiState(
		reactive(() => clientState.uiState ?? uiStateSlice.getInitialState()),
		clientState.dispatch,
	);

	// ============================================================================
	// CONFIGURATION & SETTINGS
	// ============================================================================

	const projectsService = new ProjectsService(clientState.backendApi, homeDir, backend);
	const gitConfig = new GitConfigService(clientState.backendApi, clientState.dispatch, backend);

	// ============================================================================
	// AI SERVICES
	// ============================================================================

	const aiPromptService = new AIPromptService();
	const aiService = new AIService(gitConfig, secretsService, httpClient, tokenMemoryService);
	const claudeCodeService = new ClaudeCodeService(backend, clientState.backendApi);
	const userService = new UserService(backend, httpClient, tokenMemoryService, posthog, uiState);
	const ircApiService = new IrcApiService(clientState.backendApi);
	const attachmentService = new AttachmentService(clientState);

	const ircSessionBridge = new IrcSessionBridge(
		backend,
		ircApiService,
		claudeCodeService,
		settingsService,
	);

	const workingFilesBroadcast = new WorkingFilesBroadcast(backend);

	// ============================================================================
	// FORGE FACTORY
	// ============================================================================

	const forgeFactory = new DefaultForgeFactory({
		gitHubClient,
		gitLabClient,
		backendApi: clientState.backendApi,
		gitHubApi: clientState.githubApi,
		gitLabApi: clientState.gitlabApi,
		dispatch: clientState.dispatch,
		posthog,
	});

	// ============================================================================
	// GIT & VERSION CONTROL
	// ============================================================================

	const gitService = new GitService(backend, clientState.backendApi);
	const baseBranchService = new BaseBranchService(clientState.backendApi);
	const branchService = new BranchService(clientState.backendApi);
	const cherryApplyService = new CherryApplyService(clientState.backendApi);
	const remotesService = new RemotesService(backend);
	const hooksService = new HooksService(clientState.backendApi);

	// ============================================================================
	// STACKS & WORKSPACE MANAGEMENT
	// ============================================================================

	const stackService = new StackService(
		clientState.backendApi,
		clientState.dispatch,
		forgeFactory,
		uiState,
	);
	const modeService = new ModeService(clientState.backendApi);
	const rulesService = new RulesService(clientState.backendApi);
	const worktreeService = new WorktreeService(clientState.backendApi);

	// ============================================================================
	// FILE & DIFF MANAGEMENT
	// ============================================================================

	const fileService = new FileService(backend, clientState.backendApi);
	const diffService = new DiffService(clientState.backendApi);

	// ============================================================================
	// HISTORY & OPERATIONS
	// ============================================================================

	const fModeManager = new FModeManager();
	const focusManager = new FocusManager(fModeManager);
	const historyService = new HistoryService(backend, clientState.backendApi);
	const oplogService = new OplogService(clientState.backendApi);
	const commitAnalytics = new CommitAnalytics(
		stackService,
		uiState,
		worktreeService,
		rulesService,
		fModeManager,
		projectsService,
	);
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
		historyService,
	);

	// ============================================================================
	// PROJECT & DEPENDENCY MANAGEMENT
	// ============================================================================

	const dependencyService = new DependencyService(worktreeService);

	// ============================================================================
	// ACTIONS & WORKFLOWS
	// ============================================================================

	const actionService = new ActionService(clientState.backendApi, backend);
	const upstreamIntegrationService = new UpstreamIntegrationService(
		clientState.backendApi,
		stackService,
	);

	// ============================================================================
	// FEEDS & NOTIFICATIONS
	// ============================================================================

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

	const imeHandler = new IMECompositionHandler();
	const reorderDropzoneFactory = new ReorderDropzoneFactory(stackService);
	const shortcutService = new ShortcutService(backend);
	const dragStateService = new DragStateService();
	const dropzoneRegistry = new DropzoneRegistry();
	const resizeSync = new ResizeSync();

	// ============================================================================
	// SYSTEM SERVICES
	// ============================================================================

	const cliManager = new CLIManager(clientState.backendApi);
	const dataSharingService = new DataSharingService(clientState.backendApi);
	const promptService = new PromptService(backend);
	const updaterService = new UpdaterService(
		backend,
		posthog,
		shortcutService,
		Number(appSettings.ui.checkForUpdatesIntervalInSeconds) * 1000,
	);

	// ============================================================================
	// UTILITIES
	// ============================================================================

	const urlService = new URLService(backend);
	const clipboardService = new ClipboardService(backend);
	const externalLinkService = {
		open: async (url) => await urlService.openExternalUrl(url),
	} satisfies ExternalLinkService;

	// ============================================================================
	// DEPENDENCY INJECTION REGISTRATION
	// ============================================================================

	provideAll([
		[ACTION_SERVICE, actionService],
		[AI_PROMPT_SERVICE, aiPromptService],
		[AI_SERVICE, aiService],
		[APP_DISPATCH, appState.appDispatch],
		[APP_STATE, appState],
		[BACKEND, backend],
		[BASE_BRANCH_SERVICE, baseBranchService],
		[BRANCH_SERVICE, branchService],
		[CHERRY_APPLY_SERVICE, cherryApplyService],
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
		[FEED_SERVICE, feedService],
		[FILE_SERVICE, fileService],
		[FOCUS_MANAGER, focusManager],
		[GITHUB_CLIENT, gitHubClient],
		[GITHUB_USER_SERVICE, githubUserService],
		[GITLAB_USER_SERVICE, gitlabUserService],
		[GITLAB_CLIENT, gitLabClient],
		[GIT_CONFIG_SERVICE, gitConfig],
		[GIT_SERVICE, gitService],
		[HISTORY_SERVICE, historyService],
		[HOOKS_SERVICE, hooksService],
		[HTTP_CLIENT, httpClient],
		[FILE_SELECTION_MANAGER, fileSelectionManager],
		[IME_COMPOSITION_HANDLER, imeHandler],
		[IRC_API_SERVICE, ircApiService],
		[MODE_SERVICE, modeService],
		[OPLOG_SERVICE, oplogService],
		[ORGANIZATION_SERVICE, organizationService],
		[POSTHOG_WRAPPER, posthog],
		[PROJECTS_SERVICE, projectsService],
		[PROMPT_SERVICE, promptService],
		[ATTACHMENT_SERVICE, attachmentService],
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
		[EXTERNAL_LINK_SERVICE, externalLinkService],
		[IRC_SESSION_BRIDGE, ircSessionBridge],
		[WORKING_FILES_BROADCAST, workingFilesBroadcast],
	]);
}
