import { invoke } from './ipc';
import { PUBLIC_API_BASE_URL } from '$env/static/public';

const apiUrl = new URL('/api/', new URL(PUBLIC_API_BASE_URL));

function getUrl(path: string) {
	return new URL(path, apiUrl).toString();
}

export type Feedback = {
	id: number;
	user_id: number;
	feedback: string;
	context: string;
	created_at: string;
	updated_at: string;
};

export type LoginToken = {
	token: string;
	expires: string;
	url: string;
};

export class User {
	id!: number;
	name: string | undefined;
	given_name: string | undefined;
	family_name: string | undefined;
	email!: string;
	picture!: string;
	locale!: string;
	created_at!: string;
	updated_at!: string;
	access_token!: string;
	role: string | undefined;
	supporter!: boolean;
	github_access_token: string | undefined;
	github_username: string | undefined;
}

export type Project = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
};

async function parseResponseJSON(response: Response) {
	if (response.status === 204 || response.status === 205) {
		return null;
	} else if (response.status >= 400) {
		throw new Error(`HTTP Error ${response.statusText}: ${await response.text()}`);
	} else {
		return await response.json();
	}
}

export enum RequestMethod {
	GET = 'GET',
	POST = 'POST',
	PUT = 'PUT',
	PATCH = 'PATCH',
	DELETE = 'DELETE'
}

const defaultHeaders = {
	'Content-Type': 'application/json'
};

export class CloudClient {
	constructor(public fetch = window.fetch) {}

	private formatBody(body?: FormData | object) {
		if (body instanceof FormData) {
			return body;
		} else if (body) {
			return JSON.stringify(body);
		}
	}

	// TODO: consider renaming
	async makeRequest<T>(params: {
		path: string;
		method?: RequestMethod;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}): Promise<T> {
		const butlerHeaders = new Headers(defaultHeaders);

		if (params.headers) {
			Object.entries(params.headers).forEach(([key, value]) => {
				if (value) {
					butlerHeaders.set(key, value);
				} else {
					butlerHeaders.delete(key);
				}
			});
		}

		if (params.token) butlerHeaders.set('X-Auth-Token', params.token);

		const response = await this.fetch(getUrl(params.path), {
			method: params.method || RequestMethod.GET,
			headers: butlerHeaders,
			body: this.formatBody(params.body)
		});

		return parseResponseJSON(response);
	}

	async createLoginToken(): Promise<LoginToken> {
		const token = await this.makeRequest<LoginToken>({
			path: 'login/token.json',
			method: RequestMethod.POST,
			body: {}
		});
		const url = new URL(token.url);
		url.host = apiUrl.host;
		return {
			...token,
			url: url.toString()
		};
	}

	getLoginUser(token: string): Promise<User> {
		return this.makeRequest({ path: `login/user/${token}.json` });
	}

	createFeedback(
		token: string | undefined,
		params: {
			email?: string;
			message: string;
			context?: string;
			logs?: Blob | File;
			data?: Blob | File;
			repo?: Blob | File;
		}
	): Promise<Feedback> {
		const formData = new FormData();
		formData.append('message', params.message);
		if (params.email) formData.append('email', params.email);
		if (params.context) formData.append('context', params.context);
		if (params.logs) formData.append('logs', params.logs);
		if (params.repo) formData.append('repo', params.repo);
		if (params.data) formData.append('data', params.data);

		// Content Type must be unset for the right form-data border to be set automatically
		return this.makeRequest({
			path: 'feedback',
			method: RequestMethod.PUT,
			body: formData,
			headers: { 'Content-Type': undefined },
			token
		});
	}

	getUser(token: string): Promise<User> {
		return this.makeRequest({
			path: 'user.json',
			token
		});
	}

	updateUser(token: string, params: { name?: string; picture?: File }): Promise<any> {
		const formData = new FormData();
		if (params.name) formData.append('name', params.name);
		if (params.picture) formData.append('avatar', params.picture);

		// Content Type must be unset for the right form-data border to be set automatically
		return this.makeRequest({
			path: 'user.json',
			method: RequestMethod.PUT,
			body: formData,
			headers: { 'Content-Type': undefined },
			token
		});
	}

	createProject(
		token: string,
		params: {
			name: string;
			description?: string;
			uid?: string;
		}
	): Promise<Project> {
		return this.makeRequest({
			path: 'projects.json',
			method: RequestMethod.POST,
			body: params,
			token
		});
	}

	updateProject(
		token: string,
		repositoryId: string,
		params: {
			name: string;
			description?: string;
		}
	): Promise<Project> {
		return this.makeRequest({
			path: `projects/${repositoryId}.json`,
			method: RequestMethod.PUT,
			body: params,
			token
		});
	}

	listProjects(token: string): Promise<Project[]> {
		return this.makeRequest({
			path: 'projects.json',
			token
		});
	}

	getProject(token: string, repositoryId: string): Promise<Project> {
		return this.makeRequest({
			path: `projects/${repositoryId}.json`,
			token
		});
	}

	deleteProject(token: string, repositoryId: string): Promise<void> {
		return this.makeRequest({
			path: `projects/${repositoryId}.json`,
			method: RequestMethod.DELETE,
			token
		});
	}
}

export async function syncToCloud(projectId: string | undefined) {
	try {
		if (projectId) await invoke<void>('project_flush_and_push', { id: projectId });
	} catch (err: any) {
		console.error(err);
	}
}
