<script lang="ts">
	import OptionsGroup from '$components/OptionsGroup.svelte';
	import Select from '$components/Select.svelte';
	import SelectItem from '$components/SelectItem.svelte';
	import SyncButton from '$components/SyncButton.svelte';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { DesktopRoutesService } from '$lib/routes/routes.svelte';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import NotificationButton from '@gitbutler/ui/NotificationButton.svelte';
	import { goto } from '$app/navigation';

	const routes = getContext(DesktopRoutesService);
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

<div class="header">
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
				goto(routes.changeProjectPath(selectedProjectId));
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
		justify-content: space-between;
		padding: 14px 14px 14px 84px;
	}

	.traffic-lights-placeholder {
		width: 58px;
	}

	.left {
		display: flex;
		gap: 14px;
	}

	.left-buttons {
		display: flex;
		gap: 8px;
	}

	.center {
		position: absolute;
		width: 100%;
		left: 0;
		display: flex;
		justify-content: center;
		pointer-events: none;

		& * {
			pointer-events: auto;
		}
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
