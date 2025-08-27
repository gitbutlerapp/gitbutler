<script lang="ts">
	import { goto } from '$app/navigation';
	import IntegrateUpstreamModal from '$components/IntegrateUpstreamModal.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { BACKEND } from '$lib/backend';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { ircPath, projectPath, isWorkspacePath } from '$lib/routes/routes.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
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

	type Props = {
		projectId: string;
		projectTitle: string;
		actionsDisabled?: boolean;
	};

	const { projectId, projectTitle, actionsDisabled = false }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const ircService = inject(IRC_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const uiState = inject(UI_STATE);
	const modeService = inject(MODE_SERVICE);
	const baseReponse = $derived(projectId ? baseBranchService.baseBranch(projectId) : undefined);
	const base = $derived(baseReponse?.current.data);
	const settingsStore = $derived(settingsService.appSettings);
	const isWorkspace = $derived(isWorkspacePath());
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);
	const backend = inject(BACKEND);

	const mode = $derived(modeService.mode({ projectId }));
	const currentMode = $derived(mode.current.data);
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

	const isNotInWorkspace = $derived(currentBranchName !== 'gitbutler/workspace');
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
		projects.current.data?.map((project) => ({
			value: project.id,
			label: project.title
		})) || []
	);

	let newProjectLoading = $state(false);

	const unreadCount = $derived(ircService.unreadCount());
	const isNotificationsUnread = $derived(unreadCount.current > 0);

	function openModal() {
		modal?.show();
	}

	const projectState = $derived(uiState.project(projectId));
	const showingActions = $derived(projectState.showActions.current);

	function toggleButActions() {
		uiState.project(projectId).showActions.set(!showingActions);
	}
</script>

{#if projectId}
	<IntegrateUpstreamModal bind:this={modal} {projectId} />
{/if}

<div class="chrome-header" class:mac={backend.platformName === 'macos'} data-tauri-drag-region>
	<div class="chrome-left" data-tauri-drag-region>
		<div class="chrome-left-buttons" class:macos={backend.platformName === 'macos'}>
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

	<div class="chrome-center" data-tauri-drag-region>
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
						loading={newProjectLoading}
						onClick={async () => {
							newProjectLoading = true;
							try {
								const project = await projectsService.addProject();
								if (!project) {
									// User cancelled the project creation
									newProjectLoading = false;
									return;
								}
								goto(projectPath(project.id));
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
			<Tooltip text="Current branch">
				<div class="chrome-current-branch">
					<div class="chrome-current-branch__content">
						<Icon name="branch-remote" color="var(--clr-text-2)" />
						<span class="text-12 text-semibold clr-text-1">{currentBranchName}</span>
						{#if isNotInWorkspace}
							<span class="text-12 text-semibold clr-text-2"> read-only </span>
						{/if}
					</div>
				</div>
			</Tooltip>
		</div>

		{#if isNotInWorkspace}
			<Tooltip text="Switch back to gitButler/workspace">
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

	<div class="chrome-right" data-tauri-drag-region>
		{#if $ircEnabled}
			<NotificationButton
				hasUnread={isNotificationsUnread}
				onclick={() => {
					goto(ircPath(projectId));
				}}
			/>
		{/if}
		{#if canUseActions}
			<Button
				kind="outline"
				class="actions-button"
				reversedDirection
				onclick={() => {
					toggleButActions();
				}}
				disabled={actionsDisabled || !isWorkspace}
			>
				{#snippet custom()}
					<svg
						width="20"
						height="18"
						class="actions-icon"
						viewBox="0 0 20 18"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
						class:activated={showingActions}
					>
						<path
							class="actions-icon__monitor"
							d="M4 16H16C17.6569 16 19 14.6569 19 13V11.8541C19 10.7178 18.358 9.679 17.3416 9.17082L15.5528 8.27639C15.214 8.107 15 7.76074 15 7.38197V5C15 3.34315 13.6569 2 12 2H4C2.34315 2 1 3.34315 1 5V13C1 14.6569 2.34315 16 4 16Z"
							stroke-width="1.5"
						/>
						<path
							class="actions-icon__star"
							d="M7.65242 4.74446C7.76952 4.41851 8.23048 4.41851 8.34758 4.74446L8.98803 6.52723C9.23653 7.21894 9.78106 7.76348 10.4728 8.01197L12.2555 8.65242C12.5815 8.76952 12.5815 9.23048 12.2555 9.34758L10.4728 9.98803C9.78106 10.2365 9.23653 10.7811 8.98803 11.4728L8.34758 13.2555C8.23048 13.5815 7.76952 13.5815 7.65242 13.2555L7.01197 11.4728C6.76347 10.7811 6.21894 10.2365 5.52723 9.98803L3.74446 9.34758C3.41851 9.23048 3.41851 8.76952 3.74446 8.65242L5.52723 8.01197C6.21894 7.76347 6.76348 7.21894 7.01197 6.52723L7.65242 4.74446Z"
						/>

						<svg
							width="18"
							height="14"
							viewBox="0 0 18 14"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<defs>
								<linearGradient
									id="activated-gradient"
									x1="7.5"
									y1="2"
									x2="16.3281"
									y2="10.6554"
									gradientUnits="userSpaceOnUse"
								>
									<stop stop-color="#816BDA" />
									<stop offset="1" stop-color="#2EDBD2" />
								</linearGradient>
							</defs>
						</svg>
					</svg>
				{/snippet}
				Actions
			</Button>
		{/if}
	</div>
</div>

<style>
	.chrome-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px;
		overflow: hidden;
		gap: 12px;
	}

	.actions-icon {
		opacity: var(--opacity-btn-icon-outline);
	}

	.actions-icon__star,
	.actions-icon__monitor {
		transform-box: fill-box;
		transform-origin: center;
		transition:
			fill 0.2s,
			stroke 0.2s,
			transform 0.3s;
	}

	.actions-icon__star {
		fill: var(--clr-text-1);
	}

	.actions-icon__monitor {
		stroke: var(--clr-text-1);
	}

	:global(.chrome-header .actions-button) {
		&:hover:not(:disabled) .actions-icon,
		.actions-icon.activated {
			opacity: 1;

			& .actions-icon__star {
				fill: #fff;
				transform: rotate(90deg);
			}
			& .actions-icon__monitor {
				fill: url(#activated-gradient);
				stroke: url(#activated-gradient);
			}
		}
	}

	.chrome-selector-wrapper {
		display: flex;
		position: relative;
	}

	:global(.chrome-header .project-selector-btn) {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
	}

	.project-selector-btn__content {
		display: flex;
		align-items: center;
		padding-right: 2px;
		gap: 6px;
	}

	.chrome-current-branch {
		display: flex;
		align-items: center;
		padding: 0 10px 0 6px;
		border: 1px solid var(--clr-border-2);
		border-left: none;
		border-top-right-radius: 100px;
		border-bottom-right-radius: 100px;
		background-color: var(--clr-bg-2-muted);
	}

	.chrome-current-branch__content {
		display: flex;
		align-items: center;
		gap: 4px;
		opacity: 0.7;
	}

	.chrome-left {
		display: flex;
		gap: 14px;
	}

	.chrome-center {
		display: flex;
		flex-shrink: 1;
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

	/** Mac padding added here to not affect header flex-box sizing. */
	.mac .chrome-left-buttons {
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
