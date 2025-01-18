<script lang="ts">
	import '@gitbutler/ui/fonts.css';
	import '@gitbutler/ui/main.css';
	import '../styles.css';

	import AppUpdater from '$components/AppUpdater.svelte';
	import GlobalSettingsMenuAction from '$components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$components/PromptModal.svelte';
	import ReloadMenuAction from '$components/ReloadMenuAction.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import SwitchThemeMenuAction from '$components/SwitchThemeMenuAction.svelte';
	import ToastController from '$components/ToastController.svelte';
	import ZoomInOutMenuAction from '$components/ZoomInOutMenuAction.svelte';
	import { PromptService as AIPromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { CommandService, invoke } from '$lib/backend/ipc';
	import {
		IpcNameNormalizationService,
		setNameNormalizationServiceContext
	} from '$lib/branches/nameNormalizationService';
	import { AppSettings } from '$lib/config/appSettings';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { v3 } from '$lib/config/uiFeatureFlags';
	import {
		createGitHubUserServiceStore as createGitHubUserServiceStore,
		GitHubAuthenticationService,
		GitHubUserService
	} from '$lib/forge/github/githubUserService';
	import { octokitFromAccessToken } from '$lib/forge/github/octokit';
	import { HooksService } from '$lib/hooks/hooksService';
	import { platformName } from '$lib/platform/platform';
	import { ProjectsService } from '$lib/project/projectsService';
	import { PromptService } from '$lib/prompt/promptService';
	import { DesktopDispatch, DesktopState } from '$lib/redux/store.svelte';
	import { RemotesService } from '$lib/remotes/remotesService';
	import { setSecretsService } from '$lib/secrets/secretsService';
	import { SETTINGS, loadUserSettings } from '$lib/settings/userSettings';
	import { UpdaterService } from '$lib/updater/updater';
	import { User } from '$lib/user/user';
	import { UserService } from '$lib/user/userService';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { PatchService as CloudPatchService } from '@gitbutler/shared/branches/patchService';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes';
	import { UserService as CloudUserService } from '@gitbutler/shared/users/userService';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import { LineManagerFactory as StackingLineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import { onMount, setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { dev } from '$app/environment';
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { beforeNavigate, afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import { env } from '$env/dynamic/public';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const userSettings = loadUserSettings();
	setContext(SETTINGS, userSettings);

	const appState = new DesktopState();
	const feedService = new FeedService(data.cloud, appState.appDispatch);
	const organizationService = new OrganizationService(data.cloud, appState.appDispatch);
	const cloudUserService = new CloudUserService(data.cloud, appState.appDispatch);
	const cloudProjectService = new CloudProjectService(data.cloud, appState.appDispatch);
	const cloudBranchService = new CloudBranchService(data.cloud, appState.appDispatch);
	const cloudPatchService = new CloudPatchService(data.cloud, appState.appDispatch);
	const repositoryIdLookupService = new RepositoryIdLookupService(data.cloud, appState.appDispatch);
	const latestBranchLookupService = new LatestBranchLookupService(data.cloud, appState.appDispatch);
	const webRoutesService = new WebRoutesService(env.PUBLIC_CLOUD_BASE_URL);

	setContext(AppState, appState);
	setContext(AppDispatch, appState.appDispatch);
	setContext(DesktopState, appState);
	setContext(DesktopDispatch, appState.appDispatch);
	setContext(FeedService, feedService);
	setContext(OrganizationService, organizationService);
	setContext(CloudUserService, cloudUserService);
	setContext(CloudProjectService, cloudProjectService);
	setContext(CloudBranchService, cloudBranchService);
	setContext(CloudPatchService, cloudPatchService);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	setContext(LatestBranchLookupService, latestBranchLookupService);
	setContext(WebRoutesService, webRoutesService);
	setContext(HooksService, data.hooksService);
	setContext(SettingsService, data.settingsService);

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
	setContext(GitHubAuthenticationService, data.githubAuthenticationService);

	setNameNormalizationServiceContext(new IpcNameNormalizationService(invoke));

	const user = data.userService.user;
	const accessToken = $derived($user?.github_access_token);
	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);

	// Special initialization to capture pageviews for single page apps.
	if (browser) {
		beforeNavigate(() => data.posthog.capture('$pageleave'));
		afterNavigate(() => data.posthog.capture('$pageview'));
	}

	// This store is literally only used once, on GitHub oauth, to set the
	// gh username on the user object. Furthermore, it isn't used anywhere.
	// TODO: Remove the gh username completely?
	const githubUserService = $derived(octokit ? new GitHubUserService(octokit) : undefined);
	const ghUserServiceStore = createGitHubUserServiceStore(undefined);
	$effect(() => {
		ghUserServiceStore.set(githubUserService);
	});

	let shareIssueModal: ShareIssueModal;

	onMount(() => {
		return unsubscribe(
			events.on('goto', async (path: string) => await goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show())
		);
	});

	// Redirect user if v3 design feature flag does not match current url.
	function maybeRedirect(v3Enabled: boolean, path: string) {
		const projectRegex =
			/^\/(?<isV3>project\/)?(?<uuid>[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/;
		const match = projectRegex.exec(path);
		if (match?.groups) {
			const isV3 = match.groups['isV3'];
			const uuid = match.groups['uuid'];
			if (!isV3 && v3Enabled) {
				window.location.href = `/project/${uuid}`;
			} else if (isV3 && !v3Enabled) {
				window.location.href = `/${uuid}`;
			}
		}
	}

	$effect(() => {
		maybeRedirect($v3, page.url.pathname);
	});

	const handleKeyDown = createKeybind({
		// Toggle v3 design on/off
		'v 3': () => {
			$v3 = !$v3;
		}
	});
</script>

<svelte:window
	ondrop={(e) => e.preventDefault()}
	ondragover={(e) => e.preventDefault()}
	onkeydown={handleKeyDown}
/>

<div class="app-root" role="application" oncontextmenu={(e) => !dev && e.preventDefault()}>
	{#if platformName === 'macos'}
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
