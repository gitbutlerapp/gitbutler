import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import {
	addProject,
	removeProject,
	upsertProject,
	upsertProjects
} from '$lib/organizations/projectsSlice';
import { type ApiProject, apiToProject, type LoadableProject } from '$lib/organizations/types';
import { POLLING_GLACIALLY, POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { ShareLevel } from '$lib/permissions';
import type { AppDispatch } from '$lib/redux/store.svelte';

type UpdateParams = {
	slug?: string;
	name?: string;
	description?: string;
	shareLevel?: ShareLevel.Public | ShareLevel.Private;
};

type ApiUpdateParams = {
	slug?: string;
	name?: string;
	description?: string;
	share_level?: ShareLevel.Public | ShareLevel.Private;
};

function toApiUpdateParams(real: UpdateParams): ApiUpdateParams {
	return {
		slug: real.slug,
		name: real.name,
		description: real.description,
		share_level: real.shareLevel
	};
}

export class ProjectService {
	private readonly projectInterests = new InterestStore<{ repositoryId: string }>(POLLING_REGULAR);
	private readonly userProjectsInterests = new InterestStore<{ unused: 'unused' }>(
		POLLING_GLACIALLY
	);

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

	getAllProjectsInterest(): Interest {
		return this.userProjectsInterests
			.findOrCreateSubscribable({ unused: 'unused' }, async () => {
				const apiProjects = await this.httpClient.get<ApiProject[]>('projects');

				const projects: LoadableProject[] = apiProjects.map((apiProject) => ({
					status: 'found',
					id: apiProject.repository_id,
					value: apiToProject(apiProject)
				}));

				this.appDispatch.dispatch(upsertProjects(projects));
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

	async deleteProject(repositoryId: string) {
		await this.httpClient.delete(`projects/${repositoryId}`);

		this.appDispatch.dispatch(removeProject(repositoryId));
	}

	async updateProject(repositoryId: string, params: UpdateParams) {
		const apiProject = await this.httpClient.patch<ApiProject>(`projects/${repositoryId}`, {
			body: toApiUpdateParams(params)
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(
			upsertProject({ status: 'found', id: project.repositoryId, value: project })
		);
		return project;
	}
}
