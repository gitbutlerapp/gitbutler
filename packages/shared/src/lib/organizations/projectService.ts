import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { addProject, upsertProject } from '$lib/organizations/projectsSlice';
import { type ApiProject, apiToProject } from '$lib/organizations/types';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
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
				this.appDispatch.dispatch(addProject({ status: 'loading', id: repositoryId }));

				try {
					const apiProject = await this.httpClient.get<ApiProject>(`projects/${repositoryId}`);

					this.appDispatch.dispatch(
						upsertProject({ status: 'found', id: repositoryId, value: apiToProject(apiProject) })
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertProject(errorToLoadable(error, repositoryId)));
				}
			})
			.createInterest();
	}

	async createProject(name: string, description?: string) {
		const apiProject = await this.httpClient.post<ApiProject>('projects', {
			body: { name, description }
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(
			upsertProject({ status: 'found', id: project.repositoryId, value: project })
		);

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

		this.appDispatch.dispatch(
			upsertProject({ status: 'found', id: project.repositoryId, value: project })
		);

		return project;
	}
}
