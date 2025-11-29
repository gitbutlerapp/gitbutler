<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { Section, Spacer, Textarea, Textbox, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));
</script>

<Section>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<div class="fields-wrapper">
				<Textbox label="Project path" readonly id="path" value={project?.path} />
				<div class="description-wrapper">
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
				</div>
			</div>
		{/snippet}
	</ReduxResult>
</Section>

<Spacer />

<Section>
	<Section.Card labelFor="runHooks">
		{#snippet title()}
			Run Git hooks
		{/snippet}
		{#snippet caption()}
			Enabling this will run git pre-push, pre and post commit, and commit-msg hooks you have
			configured in your repository.
		{/snippet}
		{#snippet actions()}
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
		{/snippet}
	</Section.Card>
</Section>

<Spacer />

<style>
	.fields-wrapper {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 16px;
	}

	.description-wrapper {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
