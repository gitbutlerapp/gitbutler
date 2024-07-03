import type { HttpClient } from '$lib/backend/httpClient';
import type { CloudProject, Project } from '$lib/projects/types';

export class ProjectService {
	constructor(
		private project: Project,
		private httpClient: HttpClient
	) {}

	async createCloudProject(
		token: string,
		params: {
			name: string;
			description?: string;
			uid?: string;
		}
	): Promise<CloudProject> {
		return await this.httpClient.post('projects.json', {
			body: params,
			token
		});
	}

	async updateCloudProject(
		token: string,
		repositoryId: string,
		params: {
			name: string;
			description?: string;
		}
	): Promise<CloudProject> {
		return await this.httpClient.put(`projects/${repositoryId}.json`, {
			body: params,
			token
		});
	}

	async getCloudProject(token: string, repositoryId: string): Promise<CloudProject> {
		return await this.httpClient.get(`projects/${repositoryId}.json`, {
			token
		});
	}
}
