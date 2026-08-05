<script lang="ts">
	import { goto } from "$app/navigation";
	import CreateBranchModal from "$components/branch/CreateBranchModal.svelte";
	import SyncButton from "$components/forge/SyncButton.svelte";
	import IntegrateUpstreamModal from "$components/upstream/IntegrateUpstreamModal.svelte";
	import { BACKEND } from "$lib/backend";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { handleAddProjectOutcome } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { isWorkspacePath, projectPath } from "$lib/routes/routes.svelte";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
	import { inject } from "@gitbutler/core/context";
	import { Button, Icon, OptionsGroup, Select, SelectItem, TestId, Tooltip } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";

	type Props = {
		projectId: string;
		projectTitle: string;
		actionsDisabled?: boolean;
	};

	const { projectId, projectTitle, actionsDisabled = false }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const serverCapabilitiesQuery = $derived(projectsService.serverCapabilities());
	const canAddProjects = $derived(serverCapabilitiesQuery.response?.canAddProjects ?? true);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const modeService = inject(MODE_SERVICE);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const baseReponse = $derived(projectId ? baseBranchService.baseBranch(projectId) : undefined);
	const base = $derived(baseReponse?.response);
	const settingsStore = $derived(settingsService.appSettings);
	const singleBranchMode = $derived($settingsStore?.featureFlags.singleBranch ?? false);
	const useCustomTitleBar = $derived(!($settingsStore?.ui.useNativeTitleBar ?? false));
	const backend = inject(BACKEND);
	const mode = $derived(modeService.mode(projectId));
	const currentMode = $derived(mode.response);
	const currentBranchName = $derived.by(() => {
		if (currentMode?.type === "OpenWorkspace") {
			return "gitbutler/workspace";
		} else if (currentMode?.type === "OutsideWorkspace") {
			return currentMode.subject.branchName || "detached HEAD";
		} else if (currentMode?.type === "Edit") {
			return "gitbutler/edit";
		}
		return "gitbutler/workspace";
	});

	const isNotInWorkspace = $derived(
		currentMode?.type !== "OpenWorkspace" && currentMode?.type !== "Edit",
	);
	const isDetached = $derived(
		currentMode?.type === "OutsideWorkspace" && currentMode.subject.branchName === null,
	);
	const [switchBackToWorkspace, workspaceSwitch] = baseBranchService.switchBackToWorkspace;

	async function switchToWorkspace() {
		if (base) {
			await switchBackToWorkspace({
				projectId,
			});
		}
	}

	const upstreamCommits = $derived(base?.behind ?? 0);
	const isHasUpstreamCommits = $derived(upstreamCommits > 0);

	let modal = $state<ReturnType<typeof IntegrateUpstreamModal>>();

	const projects = $derived(projectsService.projects());

	const recentProjectIds = projectsService.recentProjectIds;

	// Below this number of projects a flat list is easier to scan than groups.
	const MIN_PROJECTS_FOR_GROUPING = 8;

	const mappedProjects = $derived.by(() => {
		const allProjects = projects.response ?? [];
		const recentIds = $recentProjectIds;

		const recent = recentIds
			.map((id) => allProjects.find((project) => project.id === id))
			.filter((project) => project !== undefined);
		const others = allProjects.filter((project) => !recentIds.includes(project.id));

		// No point in grouping small lists or if one of the groups is empty.
		if (
			allProjects.length < MIN_PROJECTS_FOR_GROUPING ||
			recent.length === 0 ||
			others.length === 0
		) {
			return allProjects.map((project) => ({ value: project.id, label: project.title }));
		}

		return [
			{ header: "Recent" },
			...recent.map((project) => ({ value: project.id, label: project.title })),
			{ header: "Other projects" },
			...others.map((project) => ({ value: project.id, label: project.title })),
		];
	});

	let newProjectLoading = $state(false);
	let projectSelectorOpen = $state(false);
	let newWindowModifierHeld = $state(false);

	const isMac = $derived(backend.platformName === "macos");
	// ⌘-click is the new-window gesture on macOS; elsewhere that role belongs to Ctrl.
	const newWindowModifierLabel = $derived(isMac ? "⌘" : "Ctrl");

	function hasNewWindowModifier(e: KeyboardEvent | MouseEvent) {
		return isMac ? e.metaKey : e.ctrlKey;
	}

	// Only listen while the dropdown is open, so rows can hint that the modifier opens a new window.
	$effect(() => {
		if (!projectSelectorOpen) {
			newWindowModifierHeld = false;
			return;
		}

		function update(e: KeyboardEvent | MouseEvent) {
			newWindowModifierHeld = hasNewWindowModifier(e);
		}
		function clear() {
			newWindowModifierHeld = false;
		}

		window.addEventListener("keydown", update);
		window.addEventListener("keyup", update);
		// Mouse events carry the modifier state too, so a modifier already held when the
		// dropdown opened is picked up as soon as the pointer moves over the list.
		window.addEventListener("mousemove", update);
		window.addEventListener("blur", clear);

		return () => {
			window.removeEventListener("keydown", update);
			window.removeEventListener("keyup", update);
			window.removeEventListener("mousemove", update);
			window.removeEventListener("blur", clear);
		};
	});

	const isOnWorkspacePage = $derived(!!isWorkspacePath());

	function openModal() {
		modal?.show();
	}

	let createBranchModal = $state<CreateBranchModal>();

	$effect(() => shortcutService.on("create-branch", () => createBranchModal?.show()));
	$effect(() =>
		shortcutService.on("create-dependent-branch", () => createBranchModal?.show("dependent")),
	);
</script>

{#if projectId}
	<IntegrateUpstreamModal bind:this={modal} {projectId} />
{/if}

<div
	class="chrome-header"
	class:mac={isMac}
	data-tauri-drag-region={useCustomTitleBar}
	class:single-branch={singleBranchMode}
	use:focusable
>
	<div class="chrome-left" data-tauri-drag-region={useCustomTitleBar}>
		<div class="chrome-left-buttons" class:has-traffic-lights={useCustomTitleBar}>
			<SyncButton {projectId} disabled={actionsDisabled} />

			{#if isHasUpstreamCommits}
				<Tooltip text={isDetached ? "HEAD is detached" : undefined} disabled={!isDetached}>
					<Button
						testId={TestId.IntegrateUpstreamCommitsButton}
						style="pop"
						onclick={openModal}
						disabled={!projectId || actionsDisabled || isDetached}
					>
						{upstreamCommits} upstream {upstreamCommits === 1 ? "commit" : "commits"}
					</Button>
				</Tooltip>
			{:else}
				<div class="chrome-you-are-up-to-date">
					<Icon name="tick" />
					<span class="text-12">You’re up to date</span>
				</div>
			{/if}
		</div>
	</div>

	<div class="chrome-center" data-tauri-drag-region={useCustomTitleBar}>
		<div class="chrome-selector-wrapper">
			<Select
				searchable
				value={projectId}
				options={mappedProjects}
				loading={newProjectLoading}
				disabled={newProjectLoading}
				onselect={(value: string, modifiers?) => {
					if (isMac ? modifiers?.meta : modifiers?.ctrl) {
						projectsService.openProjectInNewWindow(value);
					} else {
						goto(projectPath(value));
					}
				}}
				ontoggle={(isOpen) => (projectSelectorOpen = isOpen)}
				popupAlign="center"
				customWidth={280}
			>
				{#snippet customSelectButton()}
					<Button
						testId={TestId.ChromeHeaderProjectSelector}
						reversedDirection
						width="auto"
						kind="outline"
						isDropdown
						dropdownOpen={projectSelectorOpen}
						class="project-selector-btn"
					>
						{#snippet custom()}
							<div class="project-selector-btn__content">
								<Icon name="repo" color="var(--text-2)" />
								<span class="text-12 text-bold">{projectTitle}</span>
							</div>
						{/snippet}
					</Button>
				{/snippet}

				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem
						selected={item.value === projectId}
						{highlighted}
						hoverIcon={newWindowModifierHeld ? "open-in-folder" : undefined}
					>
						{item.label}
					</SelectItem>
				{/snippet}

				<OptionsGroup>
					{#if canAddProjects}
						<SelectItem
							icon="plus"
							testId={TestId.ChromeHeaderProjectSelectorAddLocalProject}
							loading={newProjectLoading}
							onClick={async () => {
								newProjectLoading = true;
								try {
									const outcome = await projectsService.addProject();
									if (!outcome) {
										// User cancelled the project creation
										newProjectLoading = false;
										return;
									}

									handleAddProjectOutcome(outcome, (project) => goto(projectPath(project.id)));
								} finally {
									newProjectLoading = false;
								}
							}}
						>
							Add local repository
						</SelectItem>
					{/if}
					<SelectItem
						icon="clone"
						onClick={() => {
							goto("/onboarding/clone");
						}}
					>
						Clone repository
					</SelectItem>
				</OptionsGroup>

				<div class="text-11 new-window-hint">
					<Icon name="open-in-folder" color="var(--text-3)" size={14} />
					<span>Hold {newWindowModifierLabel} to open in a new window</span>
				</div>
			</Select>
			{#if singleBranchMode}
				<Tooltip text="Current branch">
					<div class="chrome-current-branch" data-testid={TestId.ChromeHeaderCurrentBranch}>
						<div class="chrome-current-branch__content">
							<Icon name="branch" color="var(--text-2)" />
							<span class="text-12 text-bold clr-text-2 truncate">{currentBranchName}</span>
							{#if isNotInWorkspace}
								<span class="text-12 text-bold clr-text-2 op-60"> read-only </span>
							{/if}
						</div>
					</div>
				</Tooltip>
			{/if}
		</div>

		{#if currentMode && isNotInWorkspace}
			<Tooltip text="Switch back to gitbutler/workspace">
				<Button
					kind="outline"
					testId={TestId.ChromeHeaderSwitchBackToWorkspaceButton}
					icon="undo"
					style="warning"
					onclick={switchToWorkspace}
					reversedDirection
					disabled={workspaceSwitch.current.isLoading}
				>
					Back to workspace
				</Button>
			</Tooltip>
		{/if}
	</div>

	<div class="chrome-right" data-tauri-drag-region={useCustomTitleBar}>
		{#if isOnWorkspacePage}
			<Button
				testId={TestId.ChromeHeaderCreateBranchButton}
				kind="outline"
				icon="plus"
				hotkey="⌘B"
				reversedDirection
				onclick={() => createBranchModal?.show()}
			>
				Create branch
			</Button>
		{/if}
	</div>
</div>

<CreateBranchModal bind:this={createBranchModal} {projectId} />

<style>
	.chrome-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px;
		overflow: hidden;
		gap: 12px;
	}

	.chrome-selector-wrapper {
		display: flex;
		position: relative;
		overflow: hidden;
	}

	:global(.chrome-header.single-branch .project-selector-btn) {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
	}

	.project-selector-btn__content {
		display: flex;
		align-items: center;
		padding-right: 2px;
		gap: 6px;
		text-wrap: nowrap;
	}

	.chrome-current-branch {
		display: flex;
		align-items: center;
		padding: 0 10px 0 6px;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-left: none;
		border-top-right-radius: 100px;
		border-bottom-right-radius: 100px;
	}

	.chrome-current-branch__content {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 4px;
		text-wrap: nowrap;
		opacity: 0.8;
	}

	.chrome-left {
		display: flex;
		gap: 14px;
	}

	.chrome-center {
		display: flex;
		flex-shrink: 1;
		overflow: hidden;
		gap: 8px;
	}

	.chrome-right {
		display: flex;
		justify-content: right;
		gap: 4px;
	}

	/** Flex basis 0 means they grow by the same amount. */
	.chrome-right,
	.chrome-left {
		flex-grow: 1;
		flex-basis: 0;
		min-width: max-content;
	}

	.chrome-left-buttons {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.new-window-hint {
		display: flex;
		align-items: center;
		padding-inline: 12px;
		padding-block: 10px;
		gap: 8px;
		background-color: var(--bg-2);
		color: var(--text-2);
	}

	/** Mac padding added here to not affect header flex-box sizing, only applied when using custom title bar. */
	.mac .chrome-left-buttons.has-traffic-lights {
		padding-left: 70px;
	}

	.chrome-you-are-up-to-date {
		display: flex;
		align-items: center;
		padding: 0 4px;
		gap: 4px;
		color: var(--text-2);
	}
</style>
