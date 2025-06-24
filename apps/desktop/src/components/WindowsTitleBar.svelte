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
	import { editor } from '$lib/editorLink/editorLink';
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

	// Editor options mapping for display names
	const editorOptions = [
		{ schemeIdentifer: 'vscodium', displayName: 'VSCodium' },
		{ schemeIdentifer: 'vscode', displayName: 'VSCode' },
		{ schemeIdentifer: 'vscode-insiders', displayName: 'VSCode Insiders' },
		{ schemeIdentifer: 'windsurf', displayName: 'Windsurf' },
		{ schemeIdentifer: 'zed', displayName: 'Zed' },
		{ schemeIdentifer: 'cursor', displayName: 'Cursor' }
	];

	// Get display name for current editor
	const editorDisplayName = $derived(
		editorOptions.find((opt) => opt.schemeIdentifer === $editor)?.displayName || $editor
	);

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

	// Dropdown references
	let fileDropdown = $state<DropDownButton>();
	let viewDropdown = $state<DropDownButton>();
	let projectDropdown = $state<DropDownButton>();
	let helpDropdown = $state<DropDownButton>();

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
					appIcon = '/icons/nightly/128x128.png';
				} else if (import.meta.env.DEV || userAgent.includes('dev')) {
					buildType = 'dev';
					appIcon = '/icons/dev/128x128.png';
				} else {
					buildType = 'stable';
					appIcon = '/icons/128x128.png';
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

	// Always show custom title bar on Windows
	const showTitleBar = $derived(platformName === 'windows');
</script>

{#snippet editorBadgeSnippet()}
	{#if editorDisplayName}
		<Badge style="neutral" size="icon" borderRadius="var(--radius-s)">
			{editorDisplayName}
		</Badge>
	{/if}
{/snippet}

{#if showTitleBar}
	<div class="title-bar" data-tauri-drag-region>
		<!-- App Icon and Info Section -->
		<div class="title-bar__brand">
			{#if appIcon}
				<img src={appIcon} alt="GitButler" class="app-logo" width="24" height="24" />
			{/if}
			<div class="brand-info">
				<span class="app-name">GitButler</span>
				<Badge style={getBadgeStyle()}>
					{getBadgeText()}
				</Badge>
			</div>
		</div>

		<!-- Menu Items -->
		<div class="title-bar__menu" data-tauri-drag-region="false">
			<!-- File Menu -->
			<DropDownButton
				bind:this={fileDropdown}
				menuPosition="bottom"
				autoClose={true}
				onclick={() => {
					fileDropdown?.show();
				}}
			>
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
			<DropDownButton
				bind:this={viewDropdown}
				menuPosition="bottom"
				autoClose={true}
				onclick={() => {
					viewDropdown?.show();
				}}
			>
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
			<DropDownButton
				bind:this={projectDropdown}
				menuPosition="bottom"
				autoClose={true}
				onclick={() => {
					projectDropdown?.show();
				}}
			>
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
										schemeId: $editor,
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
			<DropDownButton
				bind:this={helpDropdown}
				menuPosition="bottom"
				autoClose={true}
				onclick={() => {
					helpDropdown?.show();
				}}
			>
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

		<!-- Native-style window controls -->
		<div class="native-style-controls" data-tauri-drag-region="false">
			<button
				class="native-control-button minimize"
				onclick={() => tauri.minimize?.()}
				title="Minimize"
				aria-label="Minimize window"
			>
				<svg width="10" height="10" viewBox="0 0 10 10">
					<path d="M0 5h10" stroke="currentColor" stroke-width="1" />
				</svg>
			</button>
			<button
				class="native-control-button maximize"
				onclick={() => tauri.toggleMaximize?.()}
				title="Maximize"
				aria-label="Maximize window"
			>
				<svg width="10" height="10" viewBox="0 0 10 10">
					<rect
						x="1"
						y="1"
						width="8"
						height="8"
						stroke="currentColor"
						stroke-width="1"
						fill="none"
					/>
				</svg>
			</button>
			<button
				class="native-control-button close"
				onclick={() => tauri.close?.()}
				title="Close"
				aria-label="Close window"
			>
				<svg width="10" height="10" viewBox="0 0 10 10">
					<path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1" />
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
		height: 40px; /* Increased from 32px to make title bar taller */
		padding: 0 8px;
		gap: 8px;
		background-color: var(--clr-bg-3);
		user-select: none;
	}

	.title-bar__brand {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 8px;
	}

	.app-logo {
		width: 24px;
		height: 24px;
		object-fit: contain;
		border-radius: var(--radius-s);
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
		gap: 0px;
		pointer-events: auto;
	}

	.title-bar__menu :global(.dropdown-wrapper) {
		display: flex;
	}

	/* Hide dropdown icons, separators, and vertical lines */
	.title-bar__menu :global(.dropdown-wrapper .btn .icon),
	.title-bar__menu :global(.dropdown-wrapper .btn .separator),
	.title-bar__menu :global(.dropdown-wrapper .btn svg),
	.title-bar__menu :global(.dropdown-wrapper .btn::after),
	.title-bar__menu :global(.dropdown-wrapper::after),
	.title-bar__menu :global(.separator) {
		display: none !important;
	}

	/* Style individual menu buttons as plain text */
	.title-bar__menu :global(.dropdown-wrapper .btn) {
		height: 24px;
		padding: 4px 1px;
		gap: 0 !important;
		border: none !important;
		border-radius: 0 !important;
		background: transparent !important;
		color: var(--clr-text-1);
		font-size: var(--text-11);
		cursor: pointer;
		opacity: 0.5;
		pointer-events: all !important;
		transition: opacity var(--transition-fast);
	}

	.title-bar__menu :global(.dropdown-wrapper .btn:hover) {
		opacity: 1;
	}

	.title-bar__controls-spacer {
		flex: 1;
		min-width: 20px; /* Reduced from 140px since we now have our own controls */
	}

	/* Native-style Controls - Look like Windows native controls */
	.native-style-controls {
		display: flex;
		align-items: center;
		height: 32px;
	}

	.native-control-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 46px;
		height: 32px;
		border: none;
		background: transparent;
		color: var(--clr-text-1);
		cursor: pointer;
		transition: background-color 0.1s ease;
	}

	.native-control-button:hover {
		background-color: rgba(255, 255, 255, 0.1);
	}

	.native-control-button.close:hover {
		background-color: #e81123;
		color: white;
	}

	/* Ensure content doesn't overlap with title bar - only when custom title bar is shown */
	:global(.app-root.has-custom-titlebar) {
		padding-top: 40px; /* Matches the increased title bar height */
	}

	/* Dark theme adjustments */
	:global(.dark) .title-bar {
		border-bottom-color: var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
</style>
