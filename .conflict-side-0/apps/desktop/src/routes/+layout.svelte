<script lang="ts">
	import '../styles/styles.css';
	import { browser, dev } from '$app/environment';
	import { afterNavigate, beforeNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import AppUpdater from '$components/AppUpdater.svelte';
	import FocusCursor from '$components/FocusCursor.svelte';
	import GlobalModal from '$components/GlobalModal.svelte';
	import GlobalSettingsMenuAction from '$components/GlobalSettingsMenuAction.svelte';
	import PromptModal from '$components/PromptModal.svelte';
	import ReloadMenuAction from '$components/ReloadMenuAction.svelte';
	import ReloadWarning from '$components/ReloadWarning.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import SwitchThemeMenuAction from '$components/SwitchThemeMenuAction.svelte';
	import ToastController from '$components/ToastController.svelte';
	import ZoomInOutMenuAction from '$components/ZoomInOutMenuAction.svelte';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { initDependencies } from '$lib/bootstrap/deps';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { ircEnabled, ircServer, codegenEnabled } from '$lib/config/uiFeatureFlags';
	import { GITHUB_CLIENT } from '$lib/forge/github/githubClient';
	import { IRC_CLIENT } from '$lib/irc/ircClient.svelte';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { CLIENT_STATE } from '$lib/state/clientState.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { inject } from '@gitbutler/core/context';
	import { ChipToastContainer } from '@gitbutler/ui';
	import { FOCUS_MANAGER } from '@gitbutler/ui/focus/focusManager';
	import { type Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	const { data, children }: { data: LayoutData; children: Snippet } = $props();
	const projectId = $derived(page.params.projectId);

	// =============================================================================
	// BOOTSTRAP & INIT
	// =============================================================================

	const { backend } = data;
	initDependencies(data);

	const clientState = inject(CLIENT_STATE);
	const posthog = inject(POSTHOG_WRAPPER);

	clientState.initPersist();

	// =============================================================================
	// CORE REACTIVE STATE & EFFECTS
	// =============================================================================

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	// GitHub token management
	const gitHubClient = inject(GITHUB_CLIENT);
	const accessToken = $derived($user?.github_access_token);
	$effect(() => gitHubClient.setToken(accessToken));

	// Project tracking
	const projectsService = inject(PROJECTS_SERVICE);
	$effect(() => {
		if (projectId) {
			projectsService.setLastOpenedProject(projectId);
		}
	});

	// Keyboard shortcuts
	const shortcutService = inject(SHORTCUT_SERVICE);
	$effect(() => shortcutService.listen());

	// =============================================================================
	// ANALYTICS & NAVIGATION
	// =============================================================================

	const gitConfig = inject(GIT_CONFIG_SERVICE);

	if (browser) {
		beforeNavigate(() => posthog.capture('$pageleave'));
		afterNavigate(() => {
			// Invalidate the git config on every navigation to ensure we have the latest
			// (in case the user changed something outside of GitButler)
			gitConfig.invalidateGitConfig();
			posthog.capture('$pageview');
		});
	}

	// =============================================================================
	// EXPERIMENTAL FEATURES
	// =============================================================================

	// IRC functionality (experimental)
	const ircClient = inject(IRC_CLIENT);
	const ircService = inject(IRC_SERVICE);

	$effect(() => {
		if (!$ircEnabled || !$ircServer || !$user || !$user.login) {
			return;
		}
		ircClient.connect({ server: $ircServer, nick: $user.login });
		return () => {
			ircService.disconnect();
		};
	});

	// =============================================================================
	// DEBUG & DEVELOPMENT TOOLS
	// =============================================================================

	// Debug services (only used for development)
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;
	const uiState = inject(UI_STATE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	// Debug keyboard shortcuts
	const handleKeyDown = createKeybind({
		// Toggle v3 workspace APIs on/off
		'w s 3': () => {
			settingsService.updateFeatureFlags({ ws3: !$settingsStore?.featureFlags.ws3 });
		},
		// Show commit graph visualization
		'd o t': async () => {
			const projectId = page.params.projectId;
			await backend.invoke('show_graph_svg', { projectId });
		},
		// Log environment variables
		'e n v': async () => {
			let env = await backend.invoke('env_vars');
			// eslint-disable-next-line no-console
			console.log(env);
			(window as any).tauriEnv = env;
			// eslint-disable-next-line no-console
			console.log('Also written to window.tauriEnv');
		},
		// Toggle codegen feature flag
		'c o d e g e n': () => {
			$codegenEnabled = !$codegenEnabled;
		}
	});

	const focusManager = inject(FOCUS_MANAGER);
	$effect(() => focusManager.listen());

	// Expose debugging objects to window
	(window as any)['uiState'] = uiState;
	(window as any)['idSelection'] = idSelection;
	(window as any)['clientState'] = clientState;
	(window as any)['focusManager'] = focusManager;
</script>

<svelte:window
	ondrop={(e) => e.preventDefault()}
	ondragover={(e) => e.preventDefault()}
	onkeydown={handleKeyDown}
/>

<div class="app-root" role="application" oncontextmenu={(e) => !dev && e.preventDefault()}>
	{@render children()}
</div>
<ShareIssueModal />
<ToastController />
<ChipToastContainer />
<AppUpdater />
<PromptModal />
<ZoomInOutMenuAction />
<GlobalSettingsMenuAction />
<ReloadMenuAction />
<SwitchThemeMenuAction />
<GlobalModal />
<FocusCursor />

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
