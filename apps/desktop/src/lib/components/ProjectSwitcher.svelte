<script lang="ts">
	import { ProjectService, Project } from '$lib/backend/projects';
	import OptionsGroup from '$lib/select/OptionsGroup.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext, maybeGetContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);
	const project = maybeGetContext(Project);

	const projects = $derived(projectService.projects);

	const mappedProjects = $derived(
		$projects.map((project) => ({
			value: project.id,
			label: project.title
		}))
	);

	let loading = $state(false);
	let selectedProjectId: string | undefined = $state(project ? project.id : undefined);
</script>

<div class="project-switcher">
	<Select
		value={selectedProjectId}
		options={mappedProjects}
		label="Switch to another project"
		wide
		onselect={(value) => {
			selectedProjectId = value;
		}}
		searchable
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === selectedProjectId} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}

		<OptionsGroup>
			<SelectItem
				icon="plus"
				{loading}
				on:click={async () => {
					loading = true;
					try {
						await projectService.addProject();
					} finally {
						loading = false;
					}
				}}
			>
				Add new project
			</SelectItem>
		</OptionsGroup>
	</Select>

	<Button
		style="pop"
		kind="solid"
		icon="chevron-right-small"
		disabled={selectedProjectId === project?.id}
		on:mousedown={() => {
			if (selectedProjectId) goto(`/${selectedProjectId}/`);
		}}
	>
		Open project
	</Button>
</div>

<style lang="postcss">
	.project-switcher {
		display: flex;
		flex-direction: column;
		gap: 10px;
		align-items: flex-end;
	}
</style>
