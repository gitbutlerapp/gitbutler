<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import TextArea from '$lib/components/TextArea.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { createEventDispatcher } from 'svelte';

	export let project: Project;

	let title = project?.title;
	let description = project?.description;

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();
</script>

<form class="flex flex-col gap-3">
	<fieldset class="flex flex-col gap-3">
		<div class="flex flex-col gap-1">
			<label for="path">Path</label>
			<TextBox readonly id="path" value={project?.path} />
		</div>
		<div class="flex flex-col gap-1">
			<label for="name">Project Name</label>
			<TextBox
				id="name"
				placeholder="Project name can't be empty"
				bind:value={title}
				required
				on:change={(e) => {
					project.title = e.detail;
					dispatch('updated', project);
				}}
			/>
		</div>
		<div class="flex flex-col gap-1">
			<label for="description">Project Description</label>
			<TextArea
				id="description"
				rows={3}
				bind:value={description}
				on:change={(e) => {
					project.description = e.detail;
					dispatch('updated', project);
				}}
			/>
		</div>
	</fieldset>
</form>
