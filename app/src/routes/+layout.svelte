<script lang="ts">
	import '../styles/main.css';

	import { PromptService as AIPromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { AuthService } from '$lib/backend/auth';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { HttpClient } from '$lib/backend/httpClient';
	import { ProjectService } from '$lib/backend/projects';
	import { PromptService } from '$lib/backend/prompt';
	import { UpdaterService } from '$lib/backend/updater';
	import { LineManagerFactory } from '$lib/commitLines/lineManager';
	import AppUpdater from '$lib/components/AppUpdater.svelte';
	import GlobalSettingsMenuAction from '$lib/components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$lib/components/PromptModal.svelte';
	import ShareIssueModal from '$lib/components/ShareIssueModal.svelte';
	import { GitHubService } from '$lib/github/service';
	import ToastController from '$lib/notifications/ToastController.svelte';
	import { RemotesService } from '$lib/remotes/service';
	import { SETTINGS, loadUserSettings } from '$lib/settings/userSettings';
	import { User, UserService } from '$lib/stores/user';
	import * as events from '$lib/utils/events';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { initTheme } from '$lib/utils/theme';
	import { unsubscribe } from '$lib/utils/unsubscribe';
	import { onMount, setContext } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import type { LayoutData } from './$types';
	import { dev } from '$app/environment';
	import { goto } from '$app/navigation';

	export let data: LayoutData;

	const userSettings = loadUserSettings();
	initTheme(userSettings);
	setContext(SETTINGS, userSettings);

	// Setters do not need to be reactive since `data` never updates
	setContext(UserService, data.userService);
	setContext(ProjectService, data.projectService);
	setContext(UpdaterService, data.updaterService);
	setContext(GitHubService, data.githubService);
	setContext(GitConfigService, data.gitConfig);
	setContext(AIService, data.aiService);
	setContext(PromptService, data.promptService);
	setContext(AuthService, data.authService);
	setContext(HttpClient, data.cloud);
	setContext(User, data.userService.user);
	setContext(RemotesService, data.remotesService);
	setContext(AIPromptService, data.aiPromptService);
	setContext(LineManagerFactory, data.lineManagerFactory);

	let shareIssueModal: ShareIssueModal;

	$: zoom = $userSettings.zoom || 1;
	$: document.documentElement.style.fontSize = zoom + 'rem';
	$: userSettings.update((s) => ({ ...s, zoom: zoom }));

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
	on:contextmenu={(e) => !dev && e.preventDefault()}
>
	<slot />
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
