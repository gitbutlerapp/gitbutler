import { isLoading, invoke } from './ipc';
import { nanoid } from 'nanoid';
import { PUBLIC_API_BASE_URL, PUBLIC_CHAIN_API } from '$env/static/public';

const apiUrl = new URL('/api/', new URL(PUBLIC_API_BASE_URL));

const getUrl = (path: string) => new URL(path, apiUrl).toString();

const chainApiUrl = new URL(PUBLIC_CHAIN_API);

const getChainUrl = (path: string) => new URL(path, chainApiUrl).toString();

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

export type User = {
	id: number;
	name: string | undefined;
	given_name: string | undefined;
	family_name: string | undefined;
	email: string;
	picture: string;
	locale: string;
	created_at: string;
	updated_at: string;
	access_token: string;
	role: string | undefined;
	supporter: boolean;
	github_access_token: string | undefined;
	github_username: string | undefined;
};

export type Project = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
};

const parseResponseJSON = async (response: Response) => {
	if (response.status === 204 || response.status === 205) {
		return null;
	} else if (response.status >= 400) {
		throw new Error(`HTTP Error ${response.statusText}: ${await response.text()}`);
	} else {
		return await response.json();
	}
};

type FetchMiddleware = (f: typeof fetch) => typeof fetch;

const fetchWith = (fetch: typeof window.fetch, ...middlewares: FetchMiddleware[]) =>
	middlewares.reduce((f, middleware) => middleware(f), fetch);

const withRequestId: FetchMiddleware = (fetch) => async (url, options) => {
	const requestId = nanoid();
	if (!options) options = {};
	options.headers = {
		...options?.headers,
		'X-Request-Id': requestId
	};
	const result = fetch(url, options);
	return result;
};

const withLog: FetchMiddleware = (fetch) => async (url, options) => {
	const item = { name: url.toString(), startedAt: new Date() };
	try {
		isLoading.push(item);
		const resp = await fetch(url, options);
		return resp;
	} catch (e: any) {
		console.error('fetch', e);
		throw e;
	} finally {
		isLoading.pop(item);
	}
};

export function getCloudApiClient(
	{ fetch: realFetch }: { fetch: typeof window.fetch } = {
		fetch: window.fetch
	}
) {
	const fetch = fetchWith(realFetch, withRequestId, withLog);
	return {
		login: {
			token: {
				create: (): Promise<LoginToken> =>
					fetch(getUrl('login/token.json'), {
						method: 'POST',
						headers: {
							'Content-Type': 'application/json'
						},
						body: JSON.stringify({})
					})
						.then(parseResponseJSON)
						.then((token) => {
							const url = new URL(token.url);
							url.host = apiUrl.host;
							return {
								...token,
								url: url.toString()
							};
						})
			},
			user: {
				get: (token: string): Promise<User> =>
					fetch(getUrl(`login/user/${token}.json`), {
						method: 'GET'
					}).then(parseResponseJSON)
			}
		},
		feedback: {
			create: async (
				token: string | undefined,
				params: {
					email?: string;
					message: string;
					context?: string;
					logs?: Blob | File;
					data?: Blob | File;
					repo?: Blob | File;
				}
			): Promise<Feedback> => {
				const formData = new FormData();
				formData.append('message', params.message);
				if (params.email) formData.append('email', params.email);
				if (params.logs) formData.append('logs', params.logs);
				if (params.repo) formData.append('repo', params.repo);
				if (params.data) formData.append('data', params.data);
				const headers: HeadersInit = token ? { 'X-Auth-Token': token } : {};
				return fetch(getUrl(`feedback`), {
					method: 'PUT',
					headers,
					body: formData
				}).then(parseResponseJSON);
			}
		},
		user: {
			get: (token: string): Promise<User> =>
				fetch(getUrl(`user.json`), {
					method: 'GET',
					headers: {
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON),
			update: async (token: string, params: { name?: string; picture?: File }) => {
				const formData = new FormData();
				if (params.name) {
					formData.append('name', params.name);
				}
				if (params.picture) {
					formData.append('avatar', params.picture);
				}
				return fetch(getUrl(`user.json`), {
					method: 'PUT',
					headers: {
						'X-Auth-Token': token
					},
					body: formData
				}).then(parseResponseJSON);
			}
		},
		summarize: {
			commit: (
				token: string,
				params: { diff: string; uid?: string; brief?: boolean; emoji?: boolean }
			): Promise<{ message: string }> =>
				fetch(getUrl('summarize/commit.json'), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					},
					body: JSON.stringify(params)
				}).then(parseResponseJSON),
			hunk: (params: { hunk: string }): Promise<{ message: string }> =>
				fetch(getUrl('summarize_hunk/hunk.json'), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json'
						// 'X-Auth-Token': token
					},
					body: JSON.stringify(params)
				}).then(parseResponseJSON),
			branch: (token: string, params: { diff: string }): Promise<{ message: string }> =>
				fetch(getUrl('summarize_branch_name/branch.json'), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					},
					body: JSON.stringify(params)
				}).then(parseResponseJSON)
		},
		chat: {
			new: (token: string, repositoryId: string): Promise<{ id: string }> =>
				fetch(getChainUrl(`${repositoryId}/chat`), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON),
			history: (
				token: string,
				repositoryId: string,
				chatId: string
			): Promise<{ history: []; sequence: number }> =>
				fetch(getChainUrl(`${repositoryId}/chat/${chatId}`), {
					method: 'GET',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON),
			newMessage: (
				token: string,
				repositoryId: string,
				chatId: string,
				message: string
			): Promise<{ sequence: number }> =>
				fetch(getChainUrl(`${repositoryId}/chat/${chatId}`), {
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					},
					body: JSON.stringify({ text: message })
				}).then(parseResponseJSON)
		},
		projects: {
			create: (
				token: string,
				params: { name: string; description?: string; uid?: string }
			): Promise<Project> =>
				fetch(getUrl('projects.json'), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					},
					body: JSON.stringify(params)
				}).then(parseResponseJSON),
			update: (
				token: string,
				repositoryId: string,
				params: { name: string; description?: string }
			): Promise<Project> =>
				fetch(getUrl(`projects/${repositoryId}.json`), {
					method: 'PUT',
					headers: {
						'Content-Type': 'application/json',
						'X-Auth-Token': token
					},
					body: JSON.stringify(params)
				}).then(parseResponseJSON),
			list: (token: string): Promise<Project[]> =>
				fetch(getUrl('projects.json'), {
					method: 'GET',
					headers: {
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON),
			get: (token: string, repositoryId: string): Promise<Project> =>
				fetch(getUrl(`projects/${repositoryId}.json`), {
					method: 'GET',
					headers: {
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON),
			delete: (token: string, repositoryId: string): Promise<void> =>
				fetch(getUrl(`projects/${repositoryId}.json`), {
					method: 'DELETE',
					headers: {
						'X-Auth-Token': token
					}
				}).then(parseResponseJSON)
		}
	};
}

export async function syncToCloud(projectId: string | undefined) {
	try {
		if (projectId) await invoke<void>('project_flush_and_push', { id: projectId });
	} catch (err: any) {
		console.error(err);
	}
}
