<script lang="ts">
	import { goto } from '$app/navigation';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { useAddProject } from '$lib/project/useProjects.svelte';
	import { projectPath } from '$lib/routes/routes.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, OptionsGroup, Select, SelectItem } from '@gitbutler/ui';

	const { projectId }: { projectId?: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectsQuery = $derived(projectsService.projects());

	let selectedId = $state<string | undefined>(projectId);

	const mappedProjects = $derived(
		projectsQuery.response?.map((project) => ({
			value: project.id,
			label: project.title
		})) || []
	);

	let newProjectLoading = $state(false);
	let cloneProjectLoading = $state(false);

	const { addProject } = useAddProject();
</script>

<div class="project-switcher">
	<Select
		value={selectedId}
		options={mappedProjects}
		label="Switch to another project"
		wide
		onselect={(value) => {
			selectedId = value;
		}}
		searchable
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === selectedId} {highlighted}>
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
						await addProject();
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

	<Button
		style="pop"
		icon="chevron-right-small"
		disabled={selectedId === projectId}
		onclick={() => {
			if (selectedId) goto(projectPath(selectedId));
		}}
	>
		Open project
	</Button>
</div>

<style lang="postcss">
	.project-switcher {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 10px;
	}
</style>
