import type { Project } from './project';
import type { ProjectsService } from './projectsService';
import type { Readable } from 'svelte/store';

/**
 * Provides a store to an individual proejct
 *
 * Its preferable to use this service over the static Project context.
 */

export class ProjectService {
	project: Readable<Project | undefined>;

	constructor(
		projectsService: ProjectsService,
		readonly projectId: string
	) {
		this.project = projectsService.getProjectStore(projectId);
	}
}
