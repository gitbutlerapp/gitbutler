<script lang="ts">
	import Button from './Button.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import { goto } from '$app/navigation';

	export let projectService: ProjectService;
	export let project: Project | undefined;

	$: projects$ = projectService.projects$;

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
		items={$projects$}
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
		icon="chevron-right-small"
		disabled={selectValue == project}
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
		gap: var(--space-10);
		align-items: flex-end;
	}
</style>
