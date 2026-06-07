<script lang="ts">
	import { goto } from "$app/navigation";
	import RemoveProjectButton from "$components/projectSettings/RemoveProjectButton.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { inject } from "@gitbutler/core/context";

	import { CardGroup, chipToasts } from "@gitbutler/ui";
	import type { Project } from "$lib/project/project";

	const { projectId }: { projectId: string } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const { closeSettings } = useSettingsModal();

	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;

		try {
			const projects = await projectsService.fetchProjects();
			const remainingProject: Project | undefined = projects?.find((p) => p.id !== projectId);

			if (remainingProject) {
				// When another project exists, navigate to it BEFORE deleting so
				// the [projectId] layout unmounts and its queries are cleaned up.
				// Otherwise deleteProject() cache invalidation causes AppLayout's
				// getProject to refetch and fail with "project not found".
				closeSettings();
				await goto(`/${remainingProject.id}`);
				await projectsService.deleteProject(projectId);
			} else {
				// When this is the last project, we must delete first — the root
				// page would redirect back to this project if it still exists.
				await projectsService.deleteProject(projectId);
				// Refetch so the cache has the empty list before navigating —
				// otherwise the root page may see stale data and not redirect
				// to /onboarding.
				await projectsService.fetchProjects();
				closeSettings();
				await goto("/");
			}

			chipToasts.success("Project deleted");
		} finally {
			isDeleting = false;
		}
	}
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project)}
		<CardGroup.Item standalone>
			{#snippet title()}
				Remove project
			{/snippet}
			{#snippet caption()}
				Removing projects only clears configuration — your code stays safe.
			{/snippet}

			<div>
				<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
			</div>
		</CardGroup.Item>
	{/snippet}
</ReduxResult>
