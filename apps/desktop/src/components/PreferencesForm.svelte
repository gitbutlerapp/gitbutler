<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Section from '$components/Section.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { SectionCard, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project)}
		<Section gap={8}>
			<SectionCard orientation="row" labelFor="omitCertificateCheck">
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
			</SectionCard>
		</Section>
	{/snippet}
</ReduxResult>
