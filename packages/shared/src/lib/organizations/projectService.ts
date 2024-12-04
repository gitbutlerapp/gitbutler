import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { upsertProject } from '$lib/organizations/projectsSlice';
import { type ApiProject, apiToProject } from '$lib/organizations/types';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class ProjectService {
	private readonly projectInterests = new InterestStore<{ repositoryId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getProjectInterest(repositoryId: string): Interest {
		return this.projectInterests
			.findOrCreateSubscribable({ repositoryId }, async () => {
				const apiProject = await this.httpClient.get<ApiProject>(`projects/${repositoryId}`);
				const project = apiToProject(apiProject);

				this.appDispatch.dispatch(upsertProject(project));
			})
			.createInterest();
	}

	async createProject(name: string, description?: string) {
		const apiProject = await this.httpClient.post<ApiProject>('projects', {
			body: { name, description }
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(upsertProject(project));
		return project;
	}

	async connectProjectToOrganization(
		repositoryId: string,
		organizationSlug: string,
		targetRepositorySlug?: string
	) {
		const apiProject = await this.httpClient.post<ApiProject>(`projects/${repositoryId}/connect`, {
			body: {
				organization_slug: organizationSlug,
				project_slug: targetRepositorySlug
			}
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(upsertProject(project));
		return project;
	}
}
