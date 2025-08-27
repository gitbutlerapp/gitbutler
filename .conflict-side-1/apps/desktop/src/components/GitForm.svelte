<script lang="ts">
	import CommitSigningForm from '$components/CommitSigningForm.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Section from '$components/Section.svelte';
	import { BACKEND } from '$lib/backend';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';
	import { SectionCard, Spacer, Toggle } from '@gitbutler/ui';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId));
	const backend = inject(BACKEND);

	async function onForcePushClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, ok_with_force_push: value });
	}

	async function onForcePushProtectionClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, force_push_protection: value });
	}
</script>

<Section>
	<CommitSigningForm {projectId} />
	{#if backend.platformName !== 'windows'}
		<Spacer />
		<KeysForm {projectId} showProjectName={false} />
	{/if}

	<Spacer />
	<ReduxResult {projectId} result={projectResult.current}>
		{#snippet children(project)}
			<div class="stack-v">
				<SectionCard orientation="row" labelFor="allowForcePush" roundedBottom={false} {focusable}>
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
				<SectionCard
					orientation="row"
					labelFor="forcePushProtection"
					roundedTop={false}
					{focusable}
				>
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
				</SectionCard>
			</div>
		{/snippet}
	</ReduxResult>
</Section>
