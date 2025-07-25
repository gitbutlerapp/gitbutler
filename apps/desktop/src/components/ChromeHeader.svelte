<script lang="ts">
	import { goto } from '$app/navigation';
	import IntegrateUpstreamModal from '$components/IntegrateUpstreamModal.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { platformName } from '$lib/platform/platform';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { ircPath, projectPath } from '$lib/routes/routes.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import {
		Button,
		Icon,
		NotificationButton,
		OptionsGroup,
		Select,
		SelectItem
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
	const baseReponse = $derived(projectId ? baseBranchService.baseBranch(projectId) : undefined);
	const base = $derived(baseReponse?.current.data);
	const settingsStore = $derived(settingsService.appSettings);
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);

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
	let cloneProjectLoading = $state(false);

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

<div class="chrome-header" class:mac={platformName === 'macos'} data-tauri-drag-region>
	<div class="chrome-left" data-tauri-drag-region>
		<div class="chrome-left-buttons" class:macos={platformName === 'macos'}>
			<SyncButton {projectId} size="button" disabled={actionsDisabled} />
			{#if isHasUpstreamCommits}
				<Button
					testId={TestId.IntegrateUpstreamCommitsButton}
					style="pop"
					onclick={openModal}
					disabled={!projectId || actionsDisabled}
				>
					{upstreamCommits} upstream commits
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
		<Select
			searchable
			value={projectId}
			options={mappedProjects}
			loading={newProjectLoading || cloneProjectLoading}
			disabled={newProjectLoading || cloneProjectLoading}
			onselect={(value: string) => {
				goto(projectPath(value));
			}}
			popupAlign="center"
			customWidth={300}
		>
			{#snippet customSelectButton()}
				<div class="selector-series-select">
					<span class="text-13 text-bold">{projectTitle}</span>
					<div class="selector-series-select__icon"><Icon name="chevron-down-small" /></div>
				</div>
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
							await projectsService.addProject();
						} finally {
							newProjectLoading = false;
						}
					}}
				>
					Add local repository
				</SelectItem>
				<SelectItem
					icon="clone"
					loading={cloneProjectLoading}
					onClick={async () => {
						cloneProjectLoading = true;
						try {
							goto('/onboarding/clone');
						} finally {
							cloneProjectLoading = false;
						}
					}}
				>
					Clone repository
				</SelectItem>
			</OptionsGroup>
		</Select>
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
				disabled={actionsDisabled}
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
		&:hover .actions-icon,
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

	.chrome-left {
		display: flex;
		gap: 14px;
	}

	.chrome-center {
		flex-shrink: 1;
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

	.selector-series-select {
		display: flex;
		align-items: center;
		padding: 2px 4px 2px 6px;
		gap: 4px;
		color: var(--clr-text-1);
		text-wrap: nowrap;

		&:hover {
			& .selector-series-select__icon {
				color: var(--clr-text-2);
			}
		}
	}

	.selector-series-select__icon {
		display: flex;
		color: var(--clr-text-3);
		transition: opacity var(--transition-fast);
	}

	.chrome-you-are-up-to-date {
		display: flex;
		align-items: center;
		padding: 0 4px;
		gap: 4px;
		color: var(--clr-text-2);
	}
</style>
