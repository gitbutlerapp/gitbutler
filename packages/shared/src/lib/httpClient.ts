import { derived, get, type Readable } from 'svelte/store';

export const DEFAULT_HEADERS = {
	'Content-Type': 'application/json'
};

export type RequestMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

type RequestOptions = {
	headers?: Record<string, string | undefined>;
	body?: FormData | object;
};

export class HttpClient {
	readonly apiUrl: URL;

	/**
	 * Signals whether authentication is available.
	 *
	 * It should be noted that the authentication may be present but invalid.
	 */
	readonly authenticationAvailable: Readable<boolean>;

	constructor(
		public fetch = window.fetch,
		publicApiBaseUrl: string,
		private token: Readable<string | undefined>
	) {
		this.apiUrl = new URL('/api/', publicApiBaseUrl);

		this.authenticationAvailable = derived(token, (token) => !!token);
	}

	private getApiUrl(path: string) {
		return new URL(path, this.apiUrl);
	}

	private async request(
		path: string,
		opts: RequestOptions & { method: RequestMethod }
	): Promise<Response> {
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

		const token = get(this.token);
		if (token) butlerHeaders.set('X-Auth-Token', token);

		const response = await this.fetch(this.getApiUrl(path), {
			method: opts.method,
			headers: butlerHeaders,
			body: formatBody(opts.body)
		});

		return response;
	}

	private async requestJson<T>(
		path: string,
		opts: RequestOptions & { method: RequestMethod }
	): Promise<T> {
		const response = await this.request(path, opts);
		return await parseResponseJSON(response);
	}

	async get<T>(path: string, opts?: Omit<RequestOptions, 'body'>) {
		return await this.requestJson<T>(path, { ...opts, method: 'GET' });
	}

	async post<T>(path: string, opts?: RequestOptions) {
		return await this.requestJson<T>(path, { ...opts, method: 'POST' });
	}

	async put<T>(path: string, opts?: RequestOptions) {
		return await this.requestJson<T>(path, { ...opts, method: 'PUT' });
	}

	async patch<T>(path: string, opts?: RequestOptions) {
		return await this.requestJson<T>(path, { ...opts, method: 'PATCH' });
	}

	async delete<T>(path: string, opts?: RequestOptions) {
		return await this.requestJson<T>(path, { ...opts, method: 'DELETE' });
	}

	async postRaw(path: string, opts?: RequestOptions) {
		return await this.request(path, { ...opts, method: 'POST' });
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

function formatBody(body?: FormData | object) {
	if (!body) return;
	return body instanceof FormData ? body : JSON.stringify(body);
}
