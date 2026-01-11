<script lang="ts">
	import { goto } from '$app/navigation';
	import ReduxResult from '$components/ReduxResult.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { showError } from '$lib/notifications/toasts';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { inject } from '@gitbutler/core/context';

	import { CardGroup, chipToasts } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const { closeSettings } = useSettingsModal();

	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await projectsService.deleteProject(projectId);
			closeSettings();
			goto('/');
			chipToasts.success($t('settings.project.remove.success'));
		} catch (err: any) {
			console.error(err);
			showError($t('settings.project.remove.error'), err);
		} finally {
			isDeleting = false;
		}
	}
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project)}
		<CardGroup.Item standalone>
			{#snippet title()}
				{$t('settings.project.remove.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.project.remove.caption')}
			{/snippet}

			<div>
				<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
			</div>
		</CardGroup.Item>
	{/snippet}
</ReduxResult>
