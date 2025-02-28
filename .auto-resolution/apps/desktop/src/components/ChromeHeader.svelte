<script lang="ts">
	import OptionsGroup from '$components/OptionsGroup.svelte';
	import Select from '$components/Select.svelte';
	import SelectItem from '$components/SelectItem.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { projectPath } from '$lib/routes/routes.svelte';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import NotificationButton from '@gitbutler/ui/NotificationButton.svelte';
	import { goto } from '$app/navigation';

	const projectsService = getContext(ProjectsService);
	const project = maybeGetContext(Project);

	const projects = $derived(projectsService.projects);

	const mappedProjects = $derived(
		$projects?.map((project) => ({
			value: project.id,
			label: project.title
		})) || []
	);

	let selectedProjectId: string | undefined = $state(project ? project.id : undefined);

	let newProjectLoading = $state(false);
	let cloneProjectLoading = $state(false);

	let isNotificationsUnread = $state(false);
</script>

<div class="header" class:mac={platformName === 'macos'}>
	<div class="left">
		<div class="left-buttons" class:macos={platformName === 'macos'}>
			<SyncButton size="button" />
			<Button style="pop">3 upstream commits</Button>
		</div>
	</div>
	<div class="center">
		<Select
			searchable
			value={selectedProjectId}
			options={mappedProjects}
			loading={newProjectLoading || cloneProjectLoading}
			disabled={newProjectLoading || cloneProjectLoading}
			onselect={(value: string) => {
				selectedProjectId = value;
				goto(projectPath(selectedProjectId));
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
	<div class="right">
		<NotificationButton
			hasUnread={isNotificationsUnread}
			onclick={() => {
				// TODO: implement notifications
				console.log('Example of the button animation');
				isNotificationsUnread = !isNotificationsUnread;
			}}
		/>
	</div>
</div>

<style>
	.header {
		display: flex;
		padding: 14px;
		align-items: center;
		justify-content: space-between;
		overflow: hidden;
	}

	.left {
		display: flex;
		gap: 14px;
	}

	.center {
		flex-shrink: 1;
	}

	.right {
		display: flex;
		justify-content: right;
	}

	/** Flex basis 0 means they grow by the same amount. */
	.right,
	.left {
		flex-basis: 0;
		flex-grow: 1;
	}

	.left-buttons {
		display: flex;
		gap: 8px;
	}

	/** Mac padding added here to not affect header flex-box sizing. */
	.mac .left-buttons {
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
