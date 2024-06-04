<script lang="ts">
	import Button from './Button.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import { ProjectService, Project } from '$lib/backend/projects';
	import { getContext, maybeGetContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);
	const project = maybeGetContext(Project);

	const projects = projectService.projects;

	let loading = false;
	let select: Select;
	let selectValue = project;
</script>

<div class="project-switcher">
	<Select
		id="select-project"
		label="Switch to another project"
		itemId="id"
		labelId="title"
		items={$projects}
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
		gap: var(--size-10);
		align-items: flex-end;
	}
</style>
