<script lang="ts">
	import { goto } from '$app/navigation';
	import CreateBranchModal from '$components/CreateBranchModal.svelte';
	import IntegrateUpstreamModal from '$components/IntegrateUpstreamModal.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { BACKEND } from '$lib/backend';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { handleAddProjectOutcome } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { ircPath, isWorkspacePath, projectPath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { useCreateAiStack } from '$lib/stacks/createAiStack.svelte';
	import { inject } from '@gitbutler/core/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import {
		Button,
		Icon,
		NotificationButton,
		OptionsGroup,
		Select,
		SelectItem,
		TestId,
		Tooltip
	} from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';

	type Props = {
		projectId: string;
		projectTitle: string;
		actionsDisabled?: boolean;
	};

	const { projectId, projectTitle, actionsDisabled = false }: Props = $props();

	const { createAiStack } = useCreateAiStack(reactive(() => projectId));

	const projectsService = inject(PROJECTS_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const ircService = inject(IRC_SERVICE);
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
		if (currentMode?.type === 'OpenWorkspace') {
			return 'gitbutler/workspace';
		} else if (currentMode?.type === 'OutsideWorkspace') {
			return currentMode.subject.branchName || 'detached HEAD';
		} else if (currentMode?.type === 'Edit') {
			return 'gitbutler/edit';
		}
		return 'gitbutler/workspace';
	});

	const isNotInWorkspace = $derived(
		currentMode?.type !== 'OpenWorkspace' && currentMode?.type !== 'Edit'
	);
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	async function switchToWorkspace() {
		if (base) {
			await setBaseBranchTarget({
				projectId,
				branch: base.branchName
			});
		}
	}

	const upstreamCommits = $derived(base?.behind ?? 0);
	const isHasUpstreamCommits = $derived(upstreamCommits > 0);

	let modal = $state<ReturnType<typeof IntegrateUpstreamModal>>();

	const projects = $derived(projectsService.projects());

	const mappedProjects = $derived(
		projects.response?.map((project) => ({
			value: project.id,
			label: project.title
		})) || []
	);

	let newProjectLoading = $state(false);

	const unreadCount = $derived(ircService.unreadCount());
	const isNotificationsUnread = $derived(unreadCount.current > 0);

	const isOnWorkspacePage = $derived(!!isWorkspacePath());

	function openModal() {
		modal?.show();
	}

	let createBranchModal = $state<CreateBranchModal>();

	$effect(() => shortcutService.on('create-branch', () => createBranchModal?.show()));
	$effect(() =>
		shortcutService.on('create-dependent-branch', () => createBranchModal?.show('dependent'))
	);
</script>

{#if projectId}
	<IntegrateUpstreamModal bind:this={modal} {projectId} />
{/if}

<div
	class="chrome-header"
	class:mac={backend.platformName === 'macos'}
	data-tauri-drag-region={useCustomTitleBar}
	class:single-branch={singleBranchMode}
	use:focusable
>
	<div class="chrome-left" data-tauri-drag-region={useCustomTitleBar}>
		<div class="chrome-left-buttons" class:has-traffic-lights={useCustomTitleBar}>
			<SyncButton {projectId} disabled={actionsDisabled} />

			{#if isHasUpstreamCommits}
				<Button
					testId={TestId.IntegrateUpstreamCommitsButton}
					style="pop"
					onclick={openModal}
					disabled={!projectId || actionsDisabled}
				>
					{upstreamCommits} upstream {upstreamCommits === 1 ? 'commit' : 'commits'}
				</Button>
			{:else}
				<div class="chrome-you-are-up-to-date">
					<Icon name="tick-small" />
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
					if (modifiers?.meta) {
						projectsService.openProjectInNewWindow(value);
					} else {
						goto(projectPath(value));
					}
				}}
				popupAlign="center"
				customWidth={280}
			>
				{#snippet customSelectButton()}
					<Button
						testId={TestId.ChromeHeaderProjectSelector}
						reversedDirection
						width="auto"
						kind="outline"
						icon="select-chevron"
						class="project-selector-btn"
					>
						{#snippet custom()}
							<div class="project-selector-btn__content">
								<Icon name="repo-book-small" color="var(--clr-text-2)" />
								<span class="text-12 text-bold">{projectTitle}</span>
							</div>
						{/snippet}
					</Button>
				{/snippet}

				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === projectId} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}

				<OptionsGroup>
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
					<SelectItem
						icon="clone"
						onClick={() => {
							goto('/onboarding/clone');
						}}
					>
						Clone repository
					</SelectItem>
				</OptionsGroup>
			</Select>
			{#if singleBranchMode}
				<Tooltip text="Current branch">
					<div class="chrome-current-branch">
						<div class="chrome-current-branch__content">
							<Icon name="branch-remote" color="var(--clr-text-2)" />
							<span class="text-12 text-semibold clr-text-1 truncate">{currentBranchName}</span>
							{#if isNotInWorkspace}
								<span class="text-12 text-semibold clr-text-2"> read-only </span>
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
					icon="undo"
					style="warning"
					onclick={switchToWorkspace}
					reversedDirection
					disabled={targetBranchSwitch.current.isLoading}
				>
					Back to workspace
				</Button>
			</Tooltip>
		{/if}
	</div>

	<div class="chrome-right" data-tauri-drag-region={useCustomTitleBar}>
		{#if $ircEnabled}
			<NotificationButton
				hasUnread={isNotificationsUnread}
				onclick={() => {
					goto(ircPath(projectId));
				}}
			/>
		{/if}
		{#if isOnWorkspacePage}
			<Button
				testId={TestId.ChromeHeaderCreateBranchButton}
				kind="outline"
				icon="plus-small"
				hotkey="⌘B"
				reversedDirection
				onclick={() => createBranchModal?.show()}
			>
				Create branch
			</Button>
			<Button
				testId={TestId.ChromeHeaderCreateCodegenSessionButton}
				kind="outline"
				tooltip="New Codegen Session"
				icon="ai-new-session"
				onclick={() => {
					createAiStack();
				}}
			/>
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
		border: 1px solid var(--clr-border-2);
		border-left: none;
		border-top-right-radius: 100px;
		border-bottom-right-radius: 100px;
		background-color: var(--clr-bg-2-muted);
	}

	.chrome-current-branch__content {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 4px;
		text-wrap: nowrap;
		opacity: 0.7;
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

	/** Mac padding added here to not affect header flex-box sizing, only applied when using custom title bar. */
	.mac .chrome-left-buttons.has-traffic-lights {
		padding-left: 70px;
	}

	.chrome-you-are-up-to-date {
		display: flex;
		align-items: center;
		padding: 0 4px;
		gap: 4px;
		color: var(--clr-text-2);
	}
</style>
