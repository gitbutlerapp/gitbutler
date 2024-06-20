<script lang="ts">
	import Select from '../shared/Select.svelte';
	import SelectItem from '../shared/SelectItem.svelte';
	import { ProjectService, Project } from '$lib/backend/projects';
	import Button from '$lib/shared/Button.svelte';
	import { getContext, maybeGetContext } from '$lib/utils/context';
	import { derived } from 'svelte/store';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);
	const project = maybeGetContext(Project);

	type ProjectRecord = {
		id: string;
		title: string;
	};

	const mappedProjects = derived(projectService.projects, ($projects) =>
		$projects.map((project) => ({
			id: project.id,
			title: project.title
		}))
	);

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
		items={$mappedProjects}
		placeholder="Select a project..."
		wide
		bind:value={selectValue}
		bind:this={select}
	>
		<SelectItem slot="template" let:item let:selected {selected} let:highlighted {highlighted}>
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
