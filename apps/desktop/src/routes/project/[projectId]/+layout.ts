import { ProjectService } from '$lib/project/projectService';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	const { projectsService } = await parent();

	const projectId = params.projectId;
	const projectService = new ProjectService(projectsService, projectId);

	projectsService.setLastOpenedProject(projectId);

	return {
		projectService
	};
};
