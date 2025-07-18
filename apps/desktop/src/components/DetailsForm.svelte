<script lang="ts">
	import { projectRunCommitHooks } from '$lib/config/config';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const project = getContext(Project);
	const projectsService = getContext(ProjectsService);

	const runCommitHooks = $derived(projectRunCommitHooks(project.id));

	let title = $state(project?.title);
	let description = $state(project?.description);
</script>

<SectionCard>
	<form>
		<fieldset class="fields-wrapper">
			<Textbox label="Project path" readonly id="path" value={project?.path} />
			<section class="description-wrapper">
				<Textbox
					label="Project name"
					id="name"
					placeholder="Project name can't be empty"
					bind:value={title}
					required
					onchange={(value: string) => {
						project.title = value;
						projectsService.updateProject(project);
					}}
				/>
				<Textarea
					id="description"
					minRows={3}
					maxRows={6}
					placeholder="Project description"
					bind:value={description}
					oninput={(e: Event) => {
						const target = e.currentTarget as HTMLTextAreaElement;
						project.description = target.value;
						projectsService.updateProject(project);
					}}
				/>
			</section>
		</fieldset>
	</form>
</SectionCard>

<Spacer />

<SectionCard labelFor="runHooks" orientation="row">
	{#snippet title()}
		Run commit hooks
	{/snippet}
	{#snippet caption()}
		Enabling this will run any git pre and post commit hooks you have configured in your repository.
	{/snippet}
	{#snippet actions()}
		<Toggle id="runHooks" bind:checked={$runCommitHooks} />
	{/snippet}
</SectionCard>

<Spacer />

<style>
	.fields-wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.description-wrapper {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
