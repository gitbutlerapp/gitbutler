<script lang="ts">
	import type { Project } from '$lib/api';
	import { debounce } from '$lib/utils';
	import { createEventDispatcher } from 'svelte';

	export let project: Project;

	let title = project.title;
	let description = project.description;

	const onTitleInput = debounce((e: InputEvent) => {
		project.title = (e.target as HTMLInputElement).value;
		dispatch('updated', project);
	}, 300);

	const onDescriptionInput = debounce((e: InputEvent) => {
		project.description = (e.target as HTMLTextAreaElement).value;
		dispatch('updated', project);
	}, 300);

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();
</script>

<form class="flex flex-col gap-3">
	<fieldset class="flex flex-col gap-3">
		<div class="flex flex-col gap-1">
			<label for="path">Path</label>
			<input
				disabled
				id="path"
				name="path"
				type="text"
				class="w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
				value={project?.path}
			/>
		</div>
		<div class="flex flex-col gap-1">
			<label for="name">Project Name</label>
			<input
				id="name"
				name="name"
				type="text"
				class="w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
				placeholder="Project name can't be empty"
				bind:value={title}
				required
				on:input={onTitleInput}
			/>
		</div>
		<div class="flex flex-col gap-1">
			<label for="description">Project Description</label>
			<textarea
				autocomplete="off"
				autocorrect="off"
				spellcheck="false"
				id="description"
				name="description"
				rows="3"
				class="w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
				value={description}
				on:input={onDescriptionInput}
			/>
		</div>
	</fieldset>
</form>
