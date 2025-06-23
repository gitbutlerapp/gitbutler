<script lang="ts">
	import { platformName } from '$lib/platform/platform';
	import { Tauri } from '$lib/backend/tauri';
	import { getContext } from '@gitbutler/shared/context';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import KeyboardShortcutsModal from '$components/KeyboardShortcutsModal.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { settingsPath, newSettingsPath, clonePath } from '$lib/routes/routes.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getEditorUri } from '$lib/utils/url';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { Project } from '$lib/project/project';
	import { projectSettingsPath } from '$lib/routes/routes.svelte';
	import { ProjectsService } from '$lib/project/projectsService';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { shortcuts } from '$lib/utils/hotkeys';
	import { showHistoryView } from '$lib/config/config';
	import type { Writable } from 'svelte/store';

	const tauri = getContext(Tauri);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const projectsService = getContext(ProjectsService);
	const shortcutService = getContext(ShortcutService);

	// Get the current active project reactively
	let project = $state<Project | undefined>(undefined);

	// Update project when page route changes
	$effect(() => {
		// Watch the current page URL to trigger updates when navigating between projects
		$page.url.pathname;

		const updateProject = async () => {
			try {
				const activeProject = await projectsService.getActiveProject();
				project = activeProject;
			} catch (error) {
				// No active project or error getting it
				project = undefined;
			}
		};

		updateProject();
	});

	// Modal references
	let keyboardShortcutsModal = $state<KeyboardShortcutsModal>();

	// App version and build type information
	let appVersion = $state<string>('');
	let buildType = $state<'stable' | 'nightly' | 'dev'>('stable');
	let appIcon = $state<string>('');

	// Determine build type and icon based on app configuration
	$effect(() => {
		const initializeAppInfo = async () => {
			try {
				appVersion = await tauri.currentVersion();

				// Check the app identifier to determine build type
				const userAgent = navigator.userAgent.toLowerCase();
				if (userAgent.includes('nightly')) {
					buildType = 'nightly';
					appIcon = '/icons/nightly/32x32.png';
				} else if (import.meta.env.DEV || userAgent.includes('dev')) {
					buildType = 'dev';
					appIcon = '/icons/dev/32x32.png';
				} else {
					buildType = 'stable';
					appIcon = '/icons/32x32.png';
				}
			} catch (error) {
				console.error('Failed to get app version:', error);
			}
		};

		initializeAppInfo();
	});

	// Badge text for version display
	const getBadgeText = () => {
		switch (buildType) {
			case 'nightly':
				return `Nightly v${appVersion}`;
			case 'dev':
				return 'Development';
			case 'stable':
			default:
				return appVersion || 'Stable';
		}
	};

	// Badge style based on build type
	const getBadgeStyle = () => {
		switch (buildType) {
			case 'nightly':
				return 'warning';
			case 'dev':
				return 'error';
			case 'stable':
			default:
				return 'neutral';
		}
	};

	// Zoom functionality
	const MIN_ZOOM = 0.375;
	const MAX_ZOOM = 3;
	const DEFAULT_ZOOM = 1;
	const ZOOM_STEP = 0.0625;

	function setDomZoom(zoom: number) {
		document.documentElement.style.fontSize = zoom + 'rem';
	}

	function updateZoom(newZoom: number) {
		const zoom = Math.min(Math.max(newZoom, MIN_ZOOM), MAX_ZOOM);
		setDomZoom(zoom);
		userSettings.update((s) => ({ ...s, zoom }));
	}

	function zoomIn() {
		updateZoom($userSettings.zoom + ZOOM_STEP);
	}

	function zoomOut() {
		updateZoom($userSettings.zoom - ZOOM_STEP);
	}

	function resetZoom() {
		updateZoom(DEFAULT_ZOOM);
	}

	// Add local repository
	async function addLocalRepository() {
		await projectsService.addProject();
	}

	// Clone repository
	function cloneRepository() {
		goto(clonePath());
	}

	// Switch theme functionality
	function switchTheme() {
		userSettings.update((s) => ({
			...s,
			theme: s.theme === 'light' ? 'dark' : 'light'
		}));
	}

	// Project history
	function openProjectHistory() {
		$showHistoryView = !$showHistoryView;
	}

	// Developer tools
	function openDevTools() {
		if (import.meta.env.DEV) {
			// Implementation would need Tauri API call to open devtools
			console.log('Opening developer tools...');
		}
	}

	// Share debug info
	function shareDebugInfo() {
		events.emit('openSendIssueModal');
	}

	// Keyboard shortcuts
	function openKeyboardShortcuts() {
		keyboardShortcutsModal?.show();
	}

	// Register keyboard shortcuts
	shortcutService.on('add-local-repo', addLocalRepository);
	shortcutService.on('clone-repo', cloneRepository);
	shortcutService.on('global-settings', () => goto(newSettingsPath()));
	shortcutService.on('switch-theme', switchTheme);
	shortcutService.on('zoom-in', zoomIn);
	shortcutService.on('zoom-out', zoomOut);
	shortcutService.on('zoom-reset', resetZoom);
	shortcutService.on('reload', () => location.reload());
	shortcutService.on('keyboard-shortcuts', openKeyboardShortcuts);
	shortcutService.on('share-debug', shareDebugInfo);

	// Note: Project-specific shortcuts ('history', 'project-settings', 'open-in-vscode')
	// are handled by ProjectSettingsMenuAction.svelte to avoid conflicts

	// Only show on Windows
	const showTitleBar = $derived(platformName === 'windows');
</script>

{#snippet editorBadgeSnippet()}
	{#if $userSettings.defaultCodeEditor?.displayName}
		<Badge style="neutral" size="icon">
			{$userSettings.defaultCodeEditor.displayName}
		</Badge>
	{/if}
{/snippet}

{#if showTitleBar}
	<div class="title-bar" data-tauri-drag-region>
		<!-- App Icon and Info Section -->
		<div class="title-bar__brand">
			<div class="app-icon">
				{#if appIcon}
					<img src={appIcon} alt="GitButler" width="24" height="24" />
				{/if}
			</div>
			<div class="brand-info">
				<span class="app-name">GitButler</span>
				<Badge style={getBadgeStyle()}>
					{getBadgeText()}
				</Badge>
			</div>
		</div>

		<!-- Menu Items -->
		<div class="title-bar__menu">
			<!-- File Menu -->
			<DropDownButton menuPosition="bottom" autoClose={true}>
				File
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Add Local Repository"
							keyboardShortcut={shortcuts.global.open_repository.keys}
							onclick={addLocalRepository}
						/>
						<ContextMenuItem
							label="Clone Repository"
							keyboardShortcut={shortcuts.global.clone_repository.keys}
							onclick={cloneRepository}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Settings"
							keyboardShortcut="$mod+,"
							onclick={() => {
								goto(newSettingsPath());
							}}
						/>
						<ContextMenuItem
							label="Check for updates"
							onclick={async () => {
								try {
									await tauri.checkUpdate();
								} catch (error) {
									console.error('Failed to check for updates:', error);
								}
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>

			<!-- View Menu -->
			<DropDownButton menuPosition="bottom" autoClose={true}>
				View
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Switch Theme"
							keyboardShortcut={shortcuts.view.switch_theme.keys}
							onclick={switchTheme}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Zoom In"
							keyboardShortcut={shortcuts.view.zoom_in.keys}
							onclick={zoomIn}
						/>
						<ContextMenuItem
							label="Zoom Out"
							keyboardShortcut={shortcuts.view.zoom_out.keys}
							onclick={zoomOut}
						/>
						<ContextMenuItem
							label="Reset Zoom"
							keyboardShortcut={shortcuts.view.reset_zoom.keys}
							onclick={resetZoom}
						/>
					</ContextMenuSection>
					{#if import.meta.env.DEV}
						<ContextMenuSection>
							<ContextMenuItem
								label="Developer Tools"
								keyboardShortcut="$mod+Shift+C"
								onclick={openDevTools}
							/>
							<ContextMenuItem
								label="Reload View"
								keyboardShortcut={shortcuts.view.reload_view.keys}
								onclick={() => {
									location.reload();
								}}
							/>
						</ContextMenuSection>
					{/if}
				{/snippet}
			</DropDownButton>

			<!-- Project Menu -->
			<DropDownButton menuPosition="bottom" autoClose={true}>
				Project
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Project History"
							keyboardShortcut={shortcuts.project.project_history.keys}
							disabled={!project}
							onclick={openProjectHistory}
						/>
						<ContextMenuItem
							label="Open in Editor"
							disabled={!project}
							onclick={() => {
								if (project) {
									const path = getEditorUri({
										schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
										path: [project.vscodePath],
										searchParams: { windowId: '_blank' }
									});
									openExternalUrl(path);
								}
							}}
							control={editorBadgeSnippet}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Project Settings"
							disabled={!project}
							onclick={() => {
								if (project) {
									goto(projectSettingsPath(project.id));
								}
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>

			<!-- Help Menu -->
			<DropDownButton menuPosition="bottom" autoClose={true}>
				Help
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Documentation"
							onclick={() => {
								openExternalUrl('https://docs.gitbutler.com');
							}}
						/>
						<ContextMenuItem
							label="Source Code"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler');
							}}
						/>
						<ContextMenuItem
							label="Release Notes"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler/releases');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Keyboard Shortcuts" onclick={openKeyboardShortcuts} />
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Share Debug Info" onclick={shareDebugInfo} />
						<ContextMenuItem
							label="Report an Issue"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler/issues/new/choose');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Discord"
							icon="discord"
							onclick={() => {
								openExternalUrl('https://discord.com/invite/MmFkmaJ42D');
							}}
						/>
						<ContextMenuItem
							label="YouTube"
							icon="youtube"
							onclick={() => {
								openExternalUrl('https://www.youtube.com/@gitbutlerapp');
							}}
						/>
						<ContextMenuItem
							label="X"
							icon="x"
							onclick={() => {
								openExternalUrl('https://x.com/gitbutler');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Version {appVersion}" disabled onclick={() => {}} />
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		</div>

		<!-- Window Controls Spacer -->
		<div class="title-bar__controls-spacer"></div>

		<!-- Window Control Buttons -->
		<div class="window-controls">
			<button
				class="control-button minimize"
				onclick={() => tauri.minimize?.()}
				title="Minimize"
				aria-label="Minimize window"
			>
				<svg width="10" height="10" viewBox="0 0 10 10">
					<path d="M0 5h10" stroke="currentColor" stroke-width="1" />
				</svg>
			</button>
			<button
				class="control-button maximize"
				onclick={() => tauri.toggleMaximize?.()}
				title="Maximize"
				aria-label="Maximize window"
			>
				<svg width="10" height="10" viewBox="0 0 24 24">
					<path
						d="M10.71,14.71,5.41,20H10a1,1,0,0,1,0,2H4a2,2,0,0,1-1.38-.56l0,0s0,0,0,0A2,2,0,0,1,2,20V14a1,1,0,0,1,2,0v4.59l5.29-5.3a1,1,0,0,1,1.42,1.42ZM21.44,2.62s0,0,0,0l0,0A2,2,0,0,0,20,2H14a1,1,0,0,0,0,2h4.59l-5.3,5.29a1,1,0,0,0,0,1.42,1,1,0,0,0,1.42,0L20,5.41V10a1,1,0,0,0,2,0V4A2,2,0,0,0,21.44,2.62Z"
						fill="currentColor"
					/>
				</svg>
			</button>
			<button
				class="control-button close"
				onclick={() => tauri.close?.()}
				title="Close"
				aria-label="Close window"
			>
				<svg width="10" height="10" viewBox="0 0 10 10">
					<path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
				</svg>
			</button>
		</div>
	</div>
{/if}

<KeyboardShortcutsModal bind:this={keyboardShortcutsModal} />

<style lang="postcss">
	.title-bar {
		display: flex;
		z-index: var(--z-ground);
		position: fixed;
		top: 0;
		right: 0;
		left: 0;
		align-items: center;
		width: 100%;
		height: 32px;
		padding: 0 8px;
		gap: 8px;
		background-color: var(--clr-bg-1);
		user-select: none;
	}

	.title-bar__brand {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 6px; /* Reduced gap from 8px to 6px */
	}

	.app-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px; /* Reduced from 28px to 24px */
		height: 24px; /* Reduced from 28px to 24px */
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(
			--radius-m
		); /* Increased from --radius-s to --radius-m for more border radius */
		background-color: var(--clr-bg-2);
	}

	.app-icon img {
		width: 18px; /* Reduced from 20px to 18px to create less gap inside */
		height: 18px; /* Reduced from 20px to 18px to create less gap inside */
		object-fit: contain;
		border-radius: var(--radius-s); /* Added border radius to the logo itself */
	}

	.brand-info {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.app-name {
		color: var(--clr-text-1);
		font-weight: var(--weight-semibold);
		font-size: var(--text-12);
		white-space: nowrap;
	}

	.title-bar__menu {
		display: flex;
		align-items: center;
		margin-left: 12px;
		gap: 4px;
	}

	.title-bar__menu :global(.dropdown-wrapper) {
		display: flex;
	}

	/* Style the dropdown container as a unified card */
	.title-bar__menu :global(.dropdown-wrapper .dropdown) {
		display: flex;
		height: 24px;
		overflow: hidden; /* Ensures child buttons don't break the border radius */
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: transparent;
		transition: all var(--transition-fast);
	}

	.title-bar__menu :global(.dropdown-wrapper .dropdown:hover) {
		border-color: var(--clr-border-1);
		background-color: var(--clr-bg-2);
	}

	/* Style individual buttons within the dropdown to fit seamlessly */
	.title-bar__menu :global(.dropdown-wrapper .btn) {
		height: 100%;
		padding: 4px 12px;
		border: none !important; /* Remove individual button borders */
		border-radius: 0 !important; /* Remove individual button border radius */
		background: transparent !important;
		color: var(--clr-text-2);
		font-size: var(--text-11);
		transition: color var(--transition-fast);
	}

	/* Change text color on hover of the dropdown container */
	.title-bar__menu :global(.dropdown-wrapper .dropdown:hover .btn) {
		color: var(--clr-text-1);
	}

	.title-bar__controls-spacer {
		flex: 1;
		min-width: 20px; /* Reduced from 140px since we now have our own controls */
	}

	/* Window Control Buttons */
	.window-controls {
		display: flex;
		align-items: center;
		padding: 2px;
		gap: 2px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.control-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 20px;
		border: none;
		border-radius: var(--radius-s);
		background: transparent;
		color: var(--clr-text-2);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.control-button:hover {
		background-color: var(--clr-bg-3);
		color: var(--clr-text-1);
	}

	.control-button.close:hover {
		background-color: var(--clr-error);
		color: white;
	}

	.control-button svg {
		flex-shrink: 0;
	}

	/* Ensure content doesn't overlap with title bar */
	:global(.app-root) {
		padding-top: 32px;
	}

	/* Dark theme adjustments */
	:global(.dark) .title-bar {
		border-bottom-color: var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	:global(.dark) .app-icon {
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	:global(.dark) .window-controls {
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	:global(.dark) .control-button:hover {
		background-color: var(--clr-bg-3);
	}
</style>
