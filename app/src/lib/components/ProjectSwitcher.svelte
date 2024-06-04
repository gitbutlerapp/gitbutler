<script lang="ts">
	import Button from './Button.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import { ProjectService, Project } from '$lib/backend/projects';
	import { getContext, maybeGetContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);
	const project = maybeGetContext(Project);

	type ProjectRecord = {
		id: string;
		title: string;
	};

	let mappedProjects: ProjectRecord[] = [];

	projectService.projects.subscribe((projectList) => {
		// Map the projectList to fit the ProjectRecord type
		mappedProjects = projectList.map((project) => {
			return {
				id: project.id,
				title: project.title
			};
		});
	});

	let loading = false;
	let select: Select<ProjectRecord>;
	let selectValue: ProjectRecord | undefined = project;
</script>

<div class="project-switcher">
	<Select
		id="select-project"
		label="Switch to another project"
		itemId="id"
		labelId="title"
		items={mappedProjects}
		placeholder="Select a project..."
		wide
		bind:value={selectValue}
		bind:this={select}
	>
		<SelectItem slot="template" let:item let:selected {selected}>
			{item.title}
		</SelectItem>
		<SelectItem
			slot="append"
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
	</Select>
	<Button
		style="pop"
		kind="solid"
		icon="chevron-right-small"
		disabled={selectValue === project}
		on:mousedown={() => {
			if (selectValue) goto(`/${selectValue.id}/`);
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
