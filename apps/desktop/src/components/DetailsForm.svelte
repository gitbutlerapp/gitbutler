<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Spacer, Textarea, Textbox, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));
</script>

<CardGroup>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<div class="fields-wrapper">
				<Textbox
					label={$t('settings.project.details.projectPath')}
					readonly
					id="path"
					value={project?.path}
				/>
				<div class="description-wrapper">
					<Textbox
						label={$t('settings.project.details.projectName')}
						id="name"
						placeholder={$t('settings.project.details.projectNamePlaceholder')}
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
						placeholder={$t('settings.project.details.projectDescription')}
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

<CardGroup>
	<CardGroup.Item labelFor="runHooks">
		{#snippet title()}
			{$t('settings.project.details.runGitHooks.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.project.details.runGitHooks.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
		{/snippet}
	</CardGroup.Item>
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
