import { goto } from '$app/navigation';
import { showToast } from '$lib/notifications/toasts';
import { handleAddProjectOutcome } from '$lib/project/project';
import { PROJECTS_SERVICE } from '$lib/project/projectsService';
import { projectPath } from '$lib/routes/routes.svelte';
import { inject } from '@gitbutler/core/context';

export function useAddProject(onMissingOutcome?: () => void) {
	const projectsService = inject(PROJECTS_SERVICE);

	async function addProject(path?: string) {
		const outcome = await projectsService.addProject(path);

		if (outcome) {
			handleAddProjectOutcome(
				outcome,
				async (path: string) => {
					await projectsService.initGitRepository(path);
					showToast({
						title: 'Repository Initialized',
						message: `Git repository has been successfully initialized at ${path}. Loading project...`,
						style: 'info'
					});
					await addProject(path);
				},
				(project) => goto(projectPath(project.id))
			);
		} else {
			onMissingOutcome?.();
		}
	}

	return { addProject };
}
