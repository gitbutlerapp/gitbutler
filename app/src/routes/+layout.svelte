<script lang="ts">
	import '../styles/main.css';

	import { PromptService as AIPromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { AuthService } from '$lib/backend/auth';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { HttpClient } from '$lib/backend/httpClient';
	import { invoke } from '$lib/backend/ipc';
	import { ProjectService } from '$lib/backend/projects';
	import { PromptService } from '$lib/backend/prompt';
	import { UpdaterService } from '$lib/backend/updater';
	import {
		IpcNameNormalizationService,
		setNameNormalizationServiceContext
	} from '$lib/branches/nameNormalizationService';
	import AppUpdater from '$lib/components/AppUpdater.svelte';
	import GlobalSettingsMenuAction from '$lib/components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$lib/components/PromptModal.svelte';
	import ShareIssueModal from '$lib/components/ShareIssueModal.svelte';
	import {
		createGitHubUserServiceStore as createGitHubUserServiceStore,
		GitHubUserService
	} from '$lib/gitHost/github/githubUserService';
	import { octokitFromAccessToken } from '$lib/gitHost/github/octokit';
	import ToastController from '$lib/notifications/ToastController.svelte';
	import { RemotesService } from '$lib/remotes/service';
	import { setSecretsService } from '$lib/secrets/secretsService';
	import { SETTINGS, loadUserSettings } from '$lib/settings/userSettings';
	import { User, UserService } from '$lib/stores/user';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { initTheme } from '$lib/utils/theme';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { LineManagerFactory } from '@gitbutler/ui/CommitLines/lineManager';
	import { onMount, setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { dev } from '$app/environment';
	import { goto } from '$app/navigation';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();

	const userSettings = loadUserSettings();
	initTheme(userSettings);
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
	setNameNormalizationServiceContext(new IpcNameNormalizationService(invoke));

	const user = data.userService.user;
	const accessToken = $derived($user?.github_access_token);
	const octokit = $derived(accessToken ? octokitFromAccessToken(accessToken) : undefined);

	// This store is literally only used once, on GitHub oauth, to set the
	// gh username on the user object. Furthermore, it isn't used anywhere.
	// TODO: Remove the gh username completely?
	const githubUserService = $derived(octokit ? new GitHubUserService(octokit) : undefined);
	const ghUserServiceStore = createGitHubUserServiceStore(undefined);
	$effect(() => {
		ghUserServiceStore.set(githubUserService);
	});

	let shareIssueModal: ShareIssueModal;
	let zoom = $state($userSettings.zoom);

	$effect(() => {
		document.documentElement.style.fontSize = zoom + 'rem';
		userSettings.update((s) => ({ ...s, zoom: zoom }));
	});

	onMount(() => {
		return unsubscribe(
			events.on('goto', async (path: string) => await goto(path)),
			events.on('openSendIssueModal', () => shareIssueModal?.show())
		);
	});

	const handleKeyDown = createKeybind({
		'$mod+Equal': () => {
			zoom = Math.min(zoom + 0.0625, 3);
		},
		'$mod+Minus': () => {
			zoom = Math.max(zoom - 0.0625, 0.375);
		},
		'$mod+Digit0': () => {
			zoom = 1;
		},
		'$mod+T': () => {
			userSettings.update((s) => ({
				...s,
				theme: $userSettings.theme === 'light' ? 'dark' : 'light'
			}));
		},
		'$mod+R': () => {
			location.reload();
		}
	});
</script>

<svelte:window
	on:keydown={handleKeyDown}
	on:drop={(e) => e.preventDefault()}
	on:dragover={(e) => e.preventDefault()}
/>

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
<GlobalSettingsMenuAction />

<style lang="postcss">
	.app-root {
		display: flex;
		height: 100%;
		user-select: none;
		cursor: default;
	}
</style>
