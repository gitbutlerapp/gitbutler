import { apiToBranch } from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { projectTable } from '$lib/organizations/projectsSlice';
import { updateRecentlyInteractedProjectIds } from '$lib/organizations/recentlyInteractedProjectIds';
import { updateRecentlyPushedProjectIds } from '$lib/organizations/recentlyPushedProjectIds';
import {
	type ApiProject,
	apiToProject,
	type LoadableProject,
	type Project
} from '$lib/organizations/types';
import { POLLING_GLACIALLY, POLLING_REGULAR } from '$lib/polling';
import { InjectionToken } from '../context';
import type { Branch, ApiBranch } from '$lib/branches/types';
import type { HttpClient } from '$lib/network/httpClient';
import type { ShareLevel } from '$lib/permissions';
import type { AppDispatch } from '$lib/redux/store.svelte';

type UpdateParams = {
	slug?: string;
	name?: string;
	description?: string;
	shareLevel?: ShareLevel;
	readme?: string;
};

type ApiUpdateParams = {
	slug?: string;
	name?: string;
	description?: string;
	share_level?: ShareLevel;
	readme?: string;
};

function toApiUpdateParams(real: UpdateParams): ApiUpdateParams {
	return {
		slug: real.slug,
		name: real.name,
		description: real.description,
		readme: real.readme,
		share_level: real.shareLevel
	};
}

export const PROJECT_SERVICE_TOKEN = new InjectionToken<ProjectService>('ProjectService');

export class ProjectService {
	private readonly projectInterests = new InterestStore<{ repositoryId: string }>(POLLING_REGULAR);
	private readonly userProjectsInterests = new InterestStore<{ unused: 'unused' }>(
		POLLING_GLACIALLY
	);
	private readonly recentProjectsInterests = new InterestStore<{ unused: 'unused' }>(
		POLLING_GLACIALLY
	);
	private readonly recentlyPushedProjectsInterests = new InterestStore<{ unused: 'unused' }>(
		POLLING_GLACIALLY
	);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getProjectInterest(repositoryId: string): Interest {
		return this.projectInterests
			.findOrCreateSubscribable({ repositoryId }, async () => {
				this.appDispatch.dispatch(projectTable.addOne({ status: 'loading', id: repositoryId }));

				try {
					const apiProject = await this.httpClient.get<ApiProject>(`projects/${repositoryId}`);

					this.appDispatch.dispatch(
						projectTable.upsertOne({
							status: 'found',
							id: repositoryId,
							value: apiToProject(apiProject)
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(projectTable.addOne(errorToLoadable(error, repositoryId)));
				}
			})
			.createInterest();
	}

	async getProject(repositoryId: string): Promise<Project | undefined> {
		try {
			const apiProject = await this.httpClient.get<ApiProject>(`projects/${repositoryId}`);

			this.appDispatch.dispatch(
				projectTable.upsertOne({
					status: 'found',
					id: repositoryId,
					value: apiToProject(apiProject)
				})
			);

			return apiToProject(apiProject);
		} catch (_: unknown) {
			/* empty */
		}
	}

	async getProjectBySlug(slug: string): Promise<Project | undefined> {
		try {
			const apiProject = await this.httpClient.get<ApiProject>(`projects/full/${slug}`);

			this.appDispatch.dispatch(
				projectTable.upsertOne({
					status: 'found',
					id: apiProject.repository_id,
					value: apiToProject(apiProject)
				})
			);

			return apiToProject(apiProject);
		} catch (_: unknown) {
			/* empty */
		}
	}

	/**
	 * Get patch stacks for a project by its full slug
	 * @param slug The project's full slug (owner/project)
	 * @returns Array of Branch objects
	 */
	async getProjectPatchStacks(slug: string): Promise<Branch[]> {
		try {
			const apiBranches = await this.httpClient.get<ApiBranch[]>(`patch_stack/${slug}`);
			return apiBranches.map(apiToBranch);
		} catch (error) {
			console.error(`Error fetching patch stacks for ${slug}:`, error);
			return [];
		}
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

				this.appDispatch.dispatch(projectTable.upsertMany(projects));
			})
			.createInterest();
	}

	getRecentProjectsInterest(): Interest {
		return this.recentProjectsInterests
			.findOrCreateSubscribable({ unused: 'unused' }, async () => {
				const apiProjects = await this.httpClient.get<ApiProject[]>('projects/recently_interacted');

				const projects: LoadableProject[] = apiProjects.map((apiProject) => ({
					status: 'found',
					id: apiProject.repository_id,
					value: apiToProject(apiProject)
				}));

				this.appDispatch.dispatch(projectTable.upsertMany(projects));
				this.appDispatch.dispatch(
					updateRecentlyInteractedProjectIds(projects.map((project) => project.id))
				);
			})
			.createInterest();
	}

	getRecentlyPushedProjectsInterest(): Interest {
		return this.recentlyPushedProjectsInterests
			.findOrCreateSubscribable({ unused: 'unused' }, async () => {
				const apiProjects = await this.httpClient.get<ApiProject[]>('projects/recently_pushed');

				const projects: LoadableProject[] = apiProjects.map((apiProject) => ({
					status: 'found',
					id: apiProject.repository_id,
					value: apiToProject(apiProject)
				}));

				this.appDispatch.dispatch(projectTable.upsertMany(projects));
				this.appDispatch.dispatch(
					updateRecentlyPushedProjectIds(projects.map((project) => project.id))
				);
			})
			.createInterest();
	}

	async createProject(name: string, description?: string) {
		const apiProject = await this.httpClient.post<ApiProject>('projects', {
			body: { name, description }
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(
			projectTable.upsertOne({ status: 'found', id: project.repositoryId, value: project })
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
			projectTable.upsertOne({ status: 'found', id: project.repositoryId, value: project })
		);

		return project;
	}

	async deleteProject(repositoryId: string) {
		await this.httpClient.delete(`projects/${repositoryId}`);

		this.appDispatch.dispatch(projectTable.removeOne(repositoryId));
	}

	async updateProject(repositoryId: string, params: UpdateParams) {
		const apiProject = await this.httpClient.patch<ApiProject>(`projects/${repositoryId}`, {
			body: toApiUpdateParams(params)
		});
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(
			projectTable.upsertOne({ status: 'found', id: project.repositoryId, value: project })
		);
		return project;
	}

	async disconnectProject(repositoryId: string) {
		const apiProject = await this.httpClient.post<ApiProject>(
			`projects/${repositoryId}/disconnect`
		);
		const project = apiToProject(apiProject);

		this.appDispatch.dispatch(
			projectTable.upsertOne({ status: 'found', id: project.repositoryId, value: project })
		);
		return project;
	}
}
