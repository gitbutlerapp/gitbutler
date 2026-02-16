<script lang="ts">
	import ReduxResult from "$components/ReduxResult.svelte";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Spacer, Textarea, Textbox } from "@gitbutler/ui";

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
</script>

<CardGroup>
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
</CardGroup>

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
