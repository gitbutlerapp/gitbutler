<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await projectService.deleteProject(project.id);
			await projectService.reload();
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
	<svelte:fragment slot="title">Remove project</svelte:fragment>
	<svelte:fragment slot="caption">
		You can remove projects from GitButler, your code remains safe as this only clears
		configuration.
	</svelte:fragment>
	<div>
		<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
	</div>
</SectionCard>
