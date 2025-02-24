<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import SettingsSection from '$components/SettingsSection.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project)}
		<SettingsSection gap={8}>
			<CardGroup.Item standalone labelFor="omitCertificateCheck">
				{#snippet title()}
					Ignore host certificate checks
				{/snippet}
				{#snippet caption()}
					Enabling this will ignore host certificate checks when authenticating with ssh.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="omitCertificateCheck"
						checked={project.omit_certificate_check}
						onchange={async (value: boolean) => {
							await projectsService.updateProject({
								...project,
								omit_certificate_check: value
							});
						}}
					/>
				{/snippet}
			</CardGroup.Item>
		</SettingsSection>
	{/snippet}
</ReduxResult>
