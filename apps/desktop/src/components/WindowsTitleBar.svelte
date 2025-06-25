<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { Tauri } from '$lib/backend/tauri';
	import { editor } from '$lib/editorLink/editorLink';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { newSettingsPath, projectSettingsPath, clonePath } from '$lib/routes/routes.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { shortcuts } from '$lib/utils/hotkeys';
	import { openExternalUrl, getEditorUri } from '$lib/utils/url';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import type { Writable } from 'svelte/store';

	// Services and stores
	const tauri = getContext(Tauri);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const projectsService = getContext(ProjectsService);
	const shortcutService = getContext(ShortcutService);

	// Editor configuration
	const editorOptions = [
		{ schemeIdentifer: 'vscodium', displayName: 'VSCodium' },
		{ schemeIdentifer: 'vscode', displayName: 'VSCode' },
		{ schemeIdentifer: 'vscode-insiders', displayName: 'VSCode Insiders' },
		{ schemeIdentifer: 'windsurf', displayName: 'Windsurf' },
		{ schemeIdentifer: 'zed', displayName: 'Zed' },
		{ schemeIdentifer: 'cursor', displayName: 'Cursor' }
	];

	const editorDisplayName = $derived(
		editorOptions.find((opt) => opt.schemeIdentifer === $editor)?.displayName || $editor
	);

	// State
	let project = $state<Project | undefined>(undefined);
	let appVersion = $state<string>('');
	let buildType = $state<'stable' | 'nightly' | 'dev'>('stable');
	let appIcon = $state<string>('');

	// Dropdown state management
	const dropdowns = $state({
		file: { ref: undefined as DropDownButton | undefined, visible: false },
		view: { ref: undefined as DropDownButton | undefined, visible: false },
		project: { ref: undefined as DropDownButton | undefined, visible: false },
		help: { ref: undefined as DropDownButton | undefined, visible: false }
	});

	// Generic toggle function for any dropdown
	function toggleDropdown(name: keyof typeof dropdowns) {
		const dropdown = dropdowns[name];
		if (dropdown.visible) {
			dropdown.ref?.close();
			dropdown.visible = false;
		} else {
			dropdown.ref?.show();
			dropdown.visible = true;
		}
	}

	// Helper function to close dropdown and update state
	function closeDropdown(name: keyof typeof dropdowns) {
		const dropdown = dropdowns[name];
		dropdown.ref?.close();
		dropdown.visible = false;
	}

	// Update project when route changes (we need access to the current project for menus)
	$effect(() => {
		updateProjectFromRoute();
	});

	function updateProjectFromRoute() {
		void $page.url.pathname; // Reactive dependency on route changes
		projectsService
			.getActiveProject()
			.then((activeProject) => (project = activeProject))
			.catch(() => (project = undefined));
	}

	// Initialize app info
	$effect(() => {
		initializeAppInfo();
	});

	async function initializeAppInfo() {
		try {
			appVersion = await tauri.currentVersion();
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
	}

	// Helper functions
	function getBadgeStyle() {
		switch (buildType) {
			// case 'nightly':
			// 	return 'neutral';
			// case 'dev':
			// 	return 'error';
			default:
				return undefined;
		}
	}

	function getBadgeText() {
		switch (buildType) {
			case 'nightly':
				return `Nightly v${appVersion}`;
			case 'dev':
				return 'Development';
			default:
				return undefined;
		}
	}

	// Zoom functionality - matches ZoomInOutMenuAction.svelte implementation
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

	// Menu action handlers
	const menuActions = {
		addLocalRepository: () => projectsService.addProject(),
		cloneRepository: () => goto(clonePath()),
		switchTheme: () =>
			userSettings.update((s) => ({ ...s, theme: s.theme === 'light' ? 'dark' : 'light' })),
		zoomIn: () => updateZoom($userSettings.zoom + ZOOM_STEP),
		zoomOut: () => updateZoom($userSettings.zoom - ZOOM_STEP),
		resetZoom: () => updateZoom(DEFAULT_ZOOM),
		openDevTools: () => import.meta.env.DEV && console.warn('Opening developer tools...'),
		shareDebugInfo: () => events.emit('openSendIssueModal'),
		openKeyboardShortcuts: () => {
			// Keyboard shortcuts modal is handled by existing global shortcut registration
			// This allows the shortcut key itself to work without duplicating modal instances
		},
		openProjectHistory: () => events.emit('openHistory'),
		openInEditor: () => {
			if (project) {
				const path = getEditorUri({
					schemeId: $editor,
					path: [project.vscodePath],
					searchParams: { windowId: '_blank' }
				});
				openExternalUrl(path);
			}
		}
	};

	// Register keyboard shortcuts
	Object.entries({
		'add-local-repo': menuActions.addLocalRepository,
		'clone-repo': menuActions.cloneRepository,
		'global-settings': () => goto(newSettingsPath()),
		'switch-theme': menuActions.switchTheme,
		'zoom-in': menuActions.zoomIn,
		'zoom-out': menuActions.zoomOut,
		'zoom-reset': menuActions.resetZoom,
		reload: () => location.reload(),
		'keyboard-shortcuts': menuActions.openKeyboardShortcuts,
		'share-debug': menuActions.shareDebugInfo
	}).forEach(([key, handler]) => shortcutService.on(key, handler));

	// Always show custom title bar on Windows
	const showTitleBar = $derived(platformName === 'windows');
</script>

{#snippet editorBadgeSnippet()}
	{#if editorDisplayName}
		<Badge style="neutral" size="icon">
			{editorDisplayName}
		</Badge>
	{/if}
{/snippet}

{#if showTitleBar}
	<div class="title-bar" data-tauri-drag-region>
		<div class="title-bar__brand">
			{#if appIcon}
				<img src={appIcon} alt="GitButler" class="app-logo" />
			{/if}
			{#if getBadgeStyle()}
				<div class="brand-info">
					<Badge style={getBadgeStyle()}>
						{getBadgeText()}
					</Badge>
				</div>
			{/if}
		</div>

		<!-- Menu Items -->
		<div class="title-bar__menu" data-tauri-drag-region="false">
			<!-- File Menu -->
			<DropDownButton
				bind:this={dropdowns.file.ref}
				style="neutral"
				kind="ghost"
				menuPosition="bottom"
				autoClose
				onclick={() => toggleDropdown('file')}
			>
				File
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Add Local Repository"
							keyboardShortcut={shortcuts.global.open_repository.keys}
							onclick={() => {
								menuActions.addLocalRepository();
								closeDropdown('file');
							}}
						/>
						<ContextMenuItem
							label="Clone Repository"
							keyboardShortcut={shortcuts.global.clone_repository.keys}
							onclick={() => {
								menuActions.cloneRepository();
								closeDropdown('file');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Settings"
							keyboardShortcut="$mod+,"
							onclick={() => {
								goto(newSettingsPath());
								closeDropdown('file');
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
								closeDropdown('file');
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>

			<!-- View Menu -->
			<DropDownButton
				bind:this={dropdowns.view.ref}
				style="neutral"
				kind="ghost"
				menuPosition="bottom"
				autoClose
				onclick={() => toggleDropdown('view')}
			>
				View
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Switch Theme"
							keyboardShortcut={shortcuts.view.switch_theme.keys}
							onclick={() => {
								menuActions.switchTheme();
								closeDropdown('view');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Zoom In"
							keyboardShortcut={shortcuts.view.zoom_in.keys}
							onclick={() => {
								menuActions.zoomIn();
								closeDropdown('view');
							}}
						/>
						<ContextMenuItem
							label="Zoom Out"
							keyboardShortcut={shortcuts.view.zoom_out.keys}
							onclick={() => {
								menuActions.zoomOut();
								closeDropdown('view');
							}}
						/>
						<ContextMenuItem
							label="Reset Zoom"
							keyboardShortcut={shortcuts.view.reset_zoom.keys}
							onclick={() => {
								menuActions.resetZoom();
								closeDropdown('view');
							}}
						/>
					</ContextMenuSection>
					{#if import.meta.env.DEV}
						<ContextMenuSection>
							<ContextMenuItem
								label="Developer Tools"
								keyboardShortcut="$mod+Shift+C"
								onclick={() => {
									menuActions.openDevTools();
									closeDropdown('view');
								}}
							/>
							<ContextMenuItem
								label="Reload View"
								keyboardShortcut={shortcuts.view.reload_view.keys}
								onclick={() => {
									location.reload();
									closeDropdown('view');
								}}
							/>
						</ContextMenuSection>
					{/if}
				{/snippet}
			</DropDownButton>

			<!-- Project Menu -->
			<DropDownButton
				bind:this={dropdowns.project.ref}
				style="neutral"
				kind="ghost"
				menuPosition="bottom"
				autoClose
				onclick={() => toggleDropdown('project')}
			>
				Project
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Project History"
							keyboardShortcut={shortcuts.project.project_history.keys}
							disabled={!project}
							onclick={() => {
								menuActions.openProjectHistory();
								closeDropdown('project');
							}}
						/>
						<ContextMenuItem
							label="Open in Editor"
							disabled={!project}
							onclick={() => {
								menuActions.openInEditor();
								closeDropdown('project');
							}}
							control={editorBadgeSnippet}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Project Settings"
							disabled={!project}
							onclick={() => {
								if (project) goto(projectSettingsPath(project.id));
								closeDropdown('project');
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>

			<!-- Help Menu -->
			<DropDownButton
				bind:this={dropdowns.help.ref}
				style="neutral"
				kind="ghost"
				menuPosition="bottom"
				autoClose
				onclick={() => toggleDropdown('help')}
			>
				Help
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Documentation"
							onclick={() => {
								openExternalUrl('https://docs.gitbutler.com');
								closeDropdown('help');
							}}
						/>
						<ContextMenuItem
							label="Source Code"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler');
								closeDropdown('help');
							}}
						/>
						<ContextMenuItem
							label="Release Notes"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler/releases');
								closeDropdown('help');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Keyboard Shortcuts"
							onclick={() => {
								menuActions.openKeyboardShortcuts();
								closeDropdown('help');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Share Debug Info"
							onclick={() => {
								menuActions.shareDebugInfo();
								closeDropdown('help');
							}}
						/>
						<ContextMenuItem
							label="Report an Issue"
							onclick={() => {
								openExternalUrl('https://github.com/gitbutlerapp/gitbutler/issues/new/choose');
								closeDropdown('help');
							}}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Discord"
							onclick={() => {
								openExternalUrl('https://discord.com/invite/MmFkmaJ42D');
								closeDropdown('help');
							}}
						/>
						<ContextMenuItem
							label="YouTube"
							onclick={() => {
								openExternalUrl('https://www.youtube.com/@gitbutlerapp');
								closeDropdown('help');
							}}
						/>
						<ContextMenuItem
							label="X"
							onclick={() => {
								openExternalUrl('https://x.com/gitbutler');
								closeDropdown('help');
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
				type="button"
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
				type="button"
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
				type="button"
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
		height: var(--windows-title-bar-height);
		padding-left: 8px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
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
		width: 18px;
		height: 18px;
		border-radius: var(--radius-s);
	}

	.brand-info {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.title-bar__menu {
		display: flex;
		align-items: center;
		margin-left: 6px;
		pointer-events: auto;
	}

	.title-bar__menu :global(.dropdown-wrapper) {
		display: flex;
		margin: 0;
	}

	/* Hide dropdown icons, separators, and vertical lines */
	.title-bar__menu :global(.dropdown-wrapper .btn .icon),
	.title-bar__menu :global(.dropdown-wrapper .btn .separator),
	.title-bar__menu :global(.dropdown-wrapper .btn svg),
	.title-bar__menu :global(.dropdown-wrapper .btn::after),
	.title-bar__menu :global(.dropdown-wrapper::after),
	.title-bar__menu :global(.separator) {
		display: none;
	}

	/* Style individual menu buttons as plain text */
	.title-bar__menu :global(.dropdown-wrapper .btn) {
		width: fit-content;
		min-width: auto;
		height: 24px;
		padding: 4px;
		gap: 0;
		border: none;
		border-radius: 0;
		background: transparent;
		color: var(--clr-text-1);
		font-size: 12px;
		cursor: pointer;
		opacity: 0.5;
		pointer-events: all;
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
		height: 100%;
	}

	.native-control-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 46px;
		height: 100%;
		border: none;
		background: transparent;
		color: var(--clr-text-1);
		cursor: pointer;
		opacity: 0.6;
		transition:
			background-color 0.1s,
			color 0.1s,
			opacity 0.1s;
	}

	.native-control-button:hover {
		background-color: var(--clr-bg-3-muted);
		opacity: 1;
	}

	.native-control-button.close:hover {
		background-color: #e81123;
		color: white;
		opacity: 1;
	}

	/* Ensure content doesn't overlap with title bar - only when custom title bar is shown */
	:global(.app-root.has-custom-titlebar) {
		padding-top: var(--windows-title-bar-height);
	}
</style>
