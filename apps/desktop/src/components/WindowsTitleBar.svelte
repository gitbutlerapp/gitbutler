<script lang="ts">
	import { goto } from '$app/navigation';
	import DropDownButton from '$components/DropDownButton.svelte';
	import { Tauri } from '$lib/backend/tauri';
	import { showHistoryView } from '$lib/config/config';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { settingsPath, newSettingsPath, clonePath } from '$lib/routes/routes.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getEditorUri } from '$lib/utils/url';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { projectSettingsPath } from '$lib/routes/routes.svelte';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import * as events from '$lib/utils/events';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { Writable } from 'svelte/store';

	const tauri = getContext(Tauri);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const project = getContext(Project);
	const projectsService = getContext(ProjectsService);
	const shortcutService = getContext(ShortcutService);

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

	// Toggle sidebar functionality
	function toggleSidebar() {
		// Emit the toggle-sidebar event
		events.emit('toggle-sidebar');
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
		// Implementation for keyboard shortcuts modal
		console.log('Opening keyboard shortcuts...');
	}

	// Only show on Windows
	const showTitleBar = $derived(platformName === 'windows');
</script>

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
			<DropDownButton menuPosition="bottom">
				File
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Add Local Repository"
							keyboardShortcut="⌘O"
							onclick={addLocalRepository}
						/>
						<ContextMenuItem label="Clone Repository" onclick={cloneRepository} />
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem
							label="Settings"
							keyboardShortcut="⌘,"
							onclick={() => {
								goto(newSettingsPath());
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>

			<!-- View Menu -->
			<DropDownButton menuPosition="bottom">
				View
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem label="Switch Theme" keyboardShortcut="⌘T" onclick={switchTheme} />
						<ContextMenuItem
							label="Toggle Sidebar"
							keyboardShortcut="⌘\\"
							onclick={toggleSidebar}
						/>
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Zoom In" keyboardShortcut="⌘+" onclick={zoomIn} />
						<ContextMenuItem label="Zoom Out" keyboardShortcut="⌘-" onclick={zoomOut} />
						<ContextMenuItem label="Reset Zoom" keyboardShortcut="⌘0" onclick={resetZoom} />
					</ContextMenuSection>
					{#if import.meta.env.DEV}
						<ContextMenuSection>
							<ContextMenuItem
								label="Developer Tools"
								keyboardShortcut="⌘⇧C"
								onclick={openDevTools}
							/>
							<ContextMenuItem
								label="Reload View"
								keyboardShortcut="⌘R"
								onclick={() => {
									location.reload();
								}}
							/>
						</ContextMenuSection>
					{/if}
				{/snippet}
			</DropDownButton>

			{#if project}
				<!-- Project Menu -->
				<DropDownButton menuPosition="bottom">
					Project
					{#snippet contextMenuSlot()}
						<ContextMenuSection>
							<ContextMenuItem
								label="Project History"
								keyboardShortcut="⌘⇧H"
								onclick={openProjectHistory}
							/>
							<ContextMenuItem
								label="Open in Editor"
								onclick={() => {
									const path = getEditorUri({
										schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
										path: [project.vscodePath],
										searchParams: { windowId: '_blank' }
									});
									openExternalUrl(path);
								}}
							/>
						</ContextMenuSection>
						<ContextMenuSection>
							<ContextMenuItem
								label="Project Settings"
								onclick={() => {
									goto(projectSettingsPath(project.id));
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</DropDownButton>
			{/if}

			<!-- Help Menu -->
			<DropDownButton menuPosition="bottom">
				Help
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Documentation"
							onclick={() => {
								openExternalUrl('https://docs.gitbutler.com');
							}}
						/>
						<ContextMenuItem label="Keyboard Shortcuts" onclick={openKeyboardShortcuts} />
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Share Debug Info" onclick={shareDebugInfo} />
					</ContextMenuSection>
					<ContextMenuSection>
						<ContextMenuItem label="Version {appVersion}" disabled onclick={() => {}} />
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		</div>

		<!-- Window Controls Spacer -->
		<div class="title-bar__controls-spacer"></div>
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
		height: 32px;
		padding: 0 8px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		user-select: none;
	}

	.title-bar__brand {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 8px;
	}

	.app-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-2);
	}

	.app-icon img {
		width: 20px;
		height: 20px;
		object-fit: contain;
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

	.title-bar__menu :global(.dropdown-wrapper .btn) {
		height: 24px;
		padding: 4px 12px;
		border: none;
		border-radius: var(--radius-s);
		background: transparent;
		color: var(--clr-text-2);
		font-size: var(--text-11);
		transition: all var(--transition-fast);
	}

	.title-bar__menu :global(.dropdown-wrapper .btn:hover) {
		background-color: var(--clr-bg-2);
		color: var(--clr-text-1);
	}

	.title-bar__controls-spacer {
		flex: 1;
		min-width: 140px; /* Space for Windows controls */
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
</style>
