<script lang="ts">
	import SyncButton from '$components/SyncButton.svelte';
	import IntegrateUpstreamModal from '$components/v3/IntegrateUpstreamModal.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { ircPath, projectPath } from '$lib/routes/routes.svelte';
	import * as events from '$lib/utils/events';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import NotificationButton from '@gitbutler/ui/NotificationButton.svelte';
	import OptionsGroup from '@gitbutler/ui/select/OptionsGroup.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		actionsDisabled?: boolean;
	};

	const { projectId, actionsDisabled = false }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const baseBranchService = getContext(BaseBranchService);
	const ircService = getContext(IrcService);
	const project = maybeGetContext(Project);
	const selectedProjectId: string | undefined = $derived(project ? project.id : undefined);
	const baseReponse = $derived(
		selectedProjectId ? baseBranchService.baseBranch(selectedProjectId) : undefined
	);
	const base = $derived(baseReponse?.current.data);

	const projects = $derived(projectsService.projects);
	const upstreamCommits = $derived(base?.behind ?? 0);

	let modal = $state<ReturnType<typeof IntegrateUpstreamModal>>();

	const mappedProjects = $derived(
		$projects?.map((project) => ({
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
</script>

{#if selectedProjectId}
	<IntegrateUpstreamModal bind:this={modal} projectId={selectedProjectId} />
{/if}

<div class="chrome-header" class:mac={platformName === 'macos'} data-tauri-drag-region>
	<div class="chrome-left" data-tauri-drag-region>
		<div class="chrome-left-buttons" class:macos={platformName === 'macos'}>
			<SyncButton {projectId} size="button" disabled={actionsDisabled} />
			{#if upstreamCommits > 0}
				<Button style="pop" onclick={openModal} disabled={!selectedProjectId || actionsDisabled}
					>{upstreamCommits} upstream commits</Button
				>
			{/if}
		</div>
	</div>
	<div class="chrome-center" data-tauri-drag-region>
		<Select
			searchable
			value={selectedProjectId}
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
					<span class="text-13 text-bold">{project?.title}</span>
					<div class="selector-series-select__icon"><Icon name="chevron-down-small" /></div>
				</div>
			{/snippet}

			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={item.value === selectedProjectId} {highlighted}>
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
		<Button kind="ghost" icon="timeline" onclick={() => events.emit('openHistory')} />

		{#if $ircEnabled}
			<NotificationButton
				hasUnread={isNotificationsUnread}
				onclick={() => {
					goto(ircPath(projectId));
				}}
			/>
		{/if}
	</div>
</div>

<style>
	.chrome-header {
		display: flex;
		padding: 14px;
		align-items: center;
		justify-content: space-between;
		overflow: hidden;
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
		gap: 4px;
		justify-content: right;
	}

	/** Flex basis 0 means they grow by the same amount. */
	.chrome-right,
	.chrome-left {
		flex-basis: 0;
		flex-grow: 1;
	}

	.chrome-left-buttons {
		display: flex;
		gap: 8px;
	}

	/** Mac padding added here to not affect header flex-box sizing. */
	.mac .chrome-left-buttons {
		padding-left: 70px;
	}

	.selector-series-select {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 2px 4px 2px 6px;
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
</style>
