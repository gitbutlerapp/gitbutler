<script lang="ts">
	import { goto } from '$app/navigation';
	import ReduxResult from '$components/ReduxResult.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';

	import { SectionCard, chipToasts } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId));

	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await projectsService.deleteProject(projectId);
			goto('/');
			chipToasts.success('Project deleted');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			isDeleting = false;
		}
	}
</script>

<ReduxResult {projectId} result={projectResult.current}>
	{#snippet children(project)}
		<SectionCard>
			{#snippet title()}
				Remove project
			{/snippet}
			{#snippet caption()}
				Removing projects from GitButler only clears configuration — your code stays safe.
			{/snippet}
			<div>
				<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
			</div>
		</SectionCard>
	{/snippet}
</ReduxResult>
