<script lang="ts">
	import CommitSigningForm from '$components/CommitSigningForm.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SettingsSection from '$components/SettingsSection.svelte';
	import { BACKEND } from '$lib/backend';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { Section, Spacer, Toggle } from '@gitbutler/ui';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const backend = inject(BACKEND);

	async function onForcePushClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, ok_with_force_push: value });
	}

	async function onForcePushProtectionClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, force_push_protection: value });
	}
</script>

<SettingsSection>
	<CommitSigningForm {projectId} />
	{#if backend.platformName !== 'windows'}
		<Spacer />
		<KeysForm {projectId} showProjectName={false} />
	{/if}

	<Spacer />
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<Section>
				<Section.Card labelFor="allowForcePush">
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
				</Section.Card>
				<Section.Card labelFor="forcePushProtection">
					{#snippet title()}
						Force push protection
					{/snippet}
					{#snippet caption()}
						Protect remote commits during force pushes. This will use Git's safer force push flags
						to avoid overwriting remote commit history.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="forcePushProtection"
							checked={project.force_push_protection}
							onchange={(checked) => onForcePushProtectionClick(project, checked)}
						/>
					{/snippet}
				</Section.Card>
			</Section>
		{/snippet}
	</ReduxResult>
</SettingsSection>
