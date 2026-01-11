<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import SettingsSection from '$components/SettingsSection.svelte';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project)}
		<SettingsSection gap={8}>
			<CardGroup.Item standalone labelFor="omitCertificateCheck">
				{#snippet title()}
					{$t('settings.project.experimental.ignoreCertificate.title')}
				{/snippet}
				{#snippet caption()}
					{$t('settings.project.experimental.ignoreCertificate.caption')}
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
