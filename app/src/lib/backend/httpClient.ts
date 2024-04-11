import { invoke } from './ipc';
import { PUBLIC_API_BASE_URL } from '$env/static/public';

export type Feedback = {
	id: number;
	user_id: number;
	feedback: string;
	context: string;
	created_at: string;
	updated_at: string;
};

export const API_URL = new URL('/api/', PUBLIC_API_BASE_URL);
export const DEFAULT_HEADERS = {
	'Content-Type': 'application/json'
};

export type RequestMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

function getApiUrl(path: string) {
	return new URL(path, API_URL);
}

export class HttpClient {
	constructor(public fetch = window.fetch) {}

	private formatBody(body?: FormData | object) {
		if (body instanceof FormData) {
			return body;
		} else if (body) {
			return JSON.stringify(body);
		}
	}

	async request<T>(params: {
		path: string;
		method: RequestMethod;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}): Promise<T> {
		const butlerHeaders = new Headers(DEFAULT_HEADERS);

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

		const response = await this.fetch(getApiUrl(params.path), {
			method: params.method || 'GET',
			headers: butlerHeaders,
			body: this.formatBody(params.body)
		});

		return parseResponseJSON(response);
	}

	get<T>(params: { path: string; token?: string; headers?: Record<string, string | undefined> }) {
		return this.request<T>({ ...params, method: 'GET' });
	}

	post<T>(params: {
		path: string;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}) {
		return this.request<T>({ ...params, method: 'POST' });
	}

	put<T>(params: {
		path: string;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}) {
		return this.request<T>({ ...params, method: 'PUT' });
	}

	patch<T>(params: {
		path: string;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}) {
		return this.request<T>({ ...params, method: 'PATCH' });
	}

	delete<T>(params: {
		path: string;
		token?: string;
		body?: FormData | object;
		headers?: Record<string, string | undefined>;
	}) {
		return this.request<T>({ ...params, method: 'DELETE' });
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
		return this.put({
			path: 'feedback',
			body: formData,
			headers: { 'Content-Type': undefined },
			token
		});
	}
}

async function parseResponseJSON(response: Response) {
	if (response.status === 204 || response.status === 205) {
		return null;
	} else if (response.status >= 400) {
		throw new Error(`HTTP Error ${response.statusText}: ${await response.text()}`);
	} else {
		return await response.json();
	}
}

export async function syncToCloud(projectId: string | undefined) {
	try {
		if (projectId) await invoke<void>('project_flush_and_push', { id: projectId });
	} catch (err: any) {
		console.error(err);
	}
}
