<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId));

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));
</script>

<SectionCard>
	<ReduxResult {projectId} result={projectResult.current}>
		{#snippet children(project)}
			<form>
				<fieldset class="fields-wrapper">
					<Textbox label="Project path" readonly id="path" value={project?.path} />
					<section class="description-wrapper">
						<Textbox
							label="Project name"
							id="name"
							placeholder="Project name can't be empty"
							value={project.title}
							required
							onchange={(value: string) => {
								projectsService.updateProject({ ...project, title: value });
							}}
						/>
						<Textarea
							id="description"
							minRows={3}
							maxRows={6}
							placeholder="Project description"
							value={project.description}
							oninput={(e: Event) => {
								const target = e.currentTarget as HTMLTextAreaElement;
								projectsService.updateProject({ ...project, description: target.value });
							}}
						/>
					</section>
				</fieldset>
			</form>
		{/snippet}
	</ReduxResult>
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
