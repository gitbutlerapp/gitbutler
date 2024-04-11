import { PUBLIC_API_BASE_URL } from '$env/static/public';

export const API_URL = new URL('/api/', PUBLIC_API_BASE_URL);
export const DEFAULT_HEADERS = {
	'Content-Type': 'application/json'
};

export type RequestMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

type RequestOptions = {
	headers?: Record<string, string | undefined>;
	body?: FormData | object;
	token?: string;
};

export class HttpClient {
	constructor(public fetch = window.fetch) {}

	private async request<T>(
		path: string,
		opts: RequestOptions & { method: RequestMethod }
	): Promise<T> {
		const butlerHeaders = new Headers(DEFAULT_HEADERS);

		if (opts.headers) {
			Object.entries(opts.headers).forEach(([key, value]) => {
				if (value) {
					butlerHeaders.set(key, value);
				} else {
					butlerHeaders.delete(key);
				}
			});
		}

		if (opts.token) butlerHeaders.set('X-Auth-Token', opts.token);

		const response = await this.fetch(getApiUrl(path), {
			method: opts.method,
			headers: butlerHeaders,
			body: formatBody(opts.body)
		});

		return parseResponseJSON(response);
	}

	async get<T>(path: string, opts?: Omit<RequestOptions, 'body'>) {
		return await this.request<T>(path, { ...opts, method: 'GET' });
	}

	async post<T>(path: string, opts?: RequestOptions) {
		return await this.request<T>(path, { ...opts, method: 'POST' });
	}

	async put<T>(path: string, opts?: RequestOptions) {
		return this.request<T>(path, { ...opts, method: 'PUT' });
	}

	async patch<T>(path: string, opts?: RequestOptions) {
		return this.request<T>(path, { ...opts, method: 'PATCH' });
	}

	async delete<T>(path: string, opts?: RequestOptions) {
		return await this.request<T>(path, { ...opts, method: 'DELETE' });
	}
}

function getApiUrl(path: string) {
	return new URL(path, API_URL);
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

function formatBody(body?: FormData | object) {
	if (!body) return;
	return body instanceof FormData ? body : JSON.stringify(body);
}
