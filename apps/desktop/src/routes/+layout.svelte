<script lang="ts">
	import '@gitbutler/ui/fonts.css';
	import '@gitbutler/ui/main.css';
	import '../styles.css';

	import { PromptService as AIPromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { AuthService } from '$lib/backend/auth';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { HttpClient } from '$lib/backend/httpClient';
	import { invoke } from '$lib/backend/ipc';
	import { ProjectService } from '$lib/backend/projects';
	import { PromptService } from '$lib/backend/prompt';
	import { UpdaterService } from '$lib/backend/updater';
	import GlobalSettingsMenuAction from '$lib/barmenuActions/GlobalSettingsMenuAction.svelte';
	import ReloadMenuAction from '$lib/barmenuActions/ReloadMenuAction.svelte';
	import SwitchThemeMenuAction from '$lib/barmenuActions/SwitchThemeMenuAction.svelte';
	import ZoomInOutMenuAction from '$lib/barmenuActions/ZoomInOutMenuAction.svelte';
	import {
		IpcNameNormalizationService,
		setNameNormalizationServiceContext
	} from '$lib/branches/nameNormalizationService';
	import AppUpdater from '$lib/components/AppUpdater.svelte';
	import PromptModal from '$lib/components/PromptModal.svelte';
	import ShareIssueModal from '$lib/components/ShareIssueModal.svelte';
	import {
		createGitHubUserServiceStore,
		GitHubUserService
	} from '$lib/gitHost/github/githubUserService';
	import { octokitFromAccessToken } from '$lib/gitHost/github/octokit';
	import ToastController from '$lib/notifications/ToastController.svelte';
	import { RemotesService } from '$lib/remotes/service';
	import { setSecretsService } from '$lib/secrets/secretsService';
	import { SETTINGS, loadUserSettings } from '$lib/settings/userSettings';
	import { User, UserService } from '$lib/stores/user';
	import * as events from '$lib/utils/events';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
	import { LineManagerFactory as StackingLineManagerFactory } from '@gitbutler/ui/commitLinesStacking/lineManager';
	import { onMount, setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { dev } from '$app/environment';
	import { goto } from '$app/navigation';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const userSettings = loadUserSettings();
	setContext(SETTINGS, userSettings);

	// Setters do not need to be reactive since `data` never updates
	setSecretsService(data.secretsService);
	setContext(UserService, data.userService);
	setContext(ProjectService, data.projectService);
	setContext(UpdaterService, data.updaterService);
	setContext(GitConfigService, data.gitConfig);
	setContext(AIService, data.aiService);
	setContext(PromptService, data.promptService);
	setContext(AuthService, data.authService);
	setContext(HttpClient, data.cloud);
	setContext(User, data.userService.user);
	setContext(RemotesService, data.remotesService);
	setContext(AIPromptService, data.aiPromptService);
	setContext(LineManagerFactory, data.lineManagerFactory);
	setContext(StackingLineManagerFactory, data.stackingLineManagerFactory);

	setNameNormalizationServiceContext(new IpcNameNormalizationService(invoke));

	const user = data.userService.user;
	const accessToken = $derived($user?.github_access_token);
	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);

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
</script>

<svelte:window on:drop={(e) => e.preventDefault()} on:dragover={(e) => e.preventDefault()} />

<div
	data-tauri-drag-region
	class="app-root"
	role="application"
	oncontextmenu={(e) => !dev && e.preventDefault()}
>
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
</style>
