<script lang="ts">
	import KeysForm from '$components/shared/KeysForm.svelte';
	import ReduxResult from '$components/shared/ReduxResult.svelte';
	import Section from '$components/shared/Section.svelte';
	import CommitSigningForm from '$components/shared/commits/CommitSigningForm.svelte';
	import { platformName } from '$lib/platform/platform';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';
	import { SectionCard, Spacer, Toggle } from '@gitbutler/ui';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId));

	async function onForcePushClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, ok_with_force_push: value });
	}
</script>

<Section>
	<CommitSigningForm {projectId} />
	{#if platformName !== 'windows'}
		<Spacer />
		<KeysForm {projectId} showProjectName={false} />
	{/if}

	<Spacer />
	<ReduxResult {projectId} result={projectResult.current}>
		{#snippet children(project)}
			<SectionCard orientation="row" labelFor="allowForcePush">
				{#snippet title()}
					Allow force pushing
				{/snippet}
				{#snippet caption()}
					Force pushing allows GitButler to override branches even if they were pushed to remote.
					GitButler will never force push to the target branch.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="allowForcePush"
						checked={project.ok_with_force_push}
						onchange={(checked) => onForcePushClick(project, checked)}
					/>
				{/snippet}
			</SectionCard>
		{/snippet}
	</ReduxResult>
</Section>
