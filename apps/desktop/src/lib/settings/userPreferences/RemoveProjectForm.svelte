<script lang="ts">
	import { Project, ProjectsService } from '$lib/backend/projects';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { showError } from '$lib/notifications/toasts';
	import * as toasts from '$lib/utils/toasts';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await projectsService.deleteProject(project.id);
			await projectsService.reload();
			goto('/');
			toasts.success('Project deleted');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			isDeleting = false;
		}
	}
</script>

<SectionCard>
	{#snippet title()}
		Remove project
	{/snippet}
	{#snippet caption()}
		You can remove projects from GitButler, your code remains safe as this only clears
		configuration.
	{/snippet}
	<div>
		<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
	</div>
</SectionCard>
