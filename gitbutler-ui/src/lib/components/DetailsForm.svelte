<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import TextArea from '$lib/components/TextArea.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { createEventDispatcher } from 'svelte';
	import type { Project } from '$lib/backend/projects';

	export let project: Project;

	let title = project?.title;
	let description = project?.description;

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();
</script>

<SectionCard>
	<form>
		<fieldset class="fields-wrapper">
			<TextBox label="Path" readonly id="path" value={project?.path} />
			<section class="description-wrapper">
				<TextBox
					label="Project Name"
					id="name"
					placeholder="Project name can't be empty"
					bind:value={title}
					required
					on:change={(e) => {
						project.title = e.detail;
						dispatch('updated', project);
					}}
				/>
				<TextArea
					id="description"
					rows={3}
					placeholder="Project description"
					bind:value={description}
					on:change={(e) => {
						project.description = e.detail;
						dispatch('updated', project);
					}}
				/>
			</section>
		</fieldset>
	</form>
</SectionCard>
<Spacer />

<style>
	.fields-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
	}

	.description-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>
