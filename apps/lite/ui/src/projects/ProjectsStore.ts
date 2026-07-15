import { ProjectStore } from "#ui/projects/ProjectStore.ts";

export class ProjectsStore {
	private readonly projects = new Map<string, ProjectStore>();

	getProject(projectId: string): ProjectStore {
		let project = this.projects.get(projectId);
		if (!project) {
			project = new ProjectStore();
			this.projects.set(projectId, project);
		}
		return project;
	}
}
