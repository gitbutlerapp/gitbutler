import { rateLimit } from '$lib/utils/ratelimit';
import { Octokit } from '@octokit/rest';

export class GitHubClient {
	private _client: Octokit | undefined;
	private _token: string | undefined;

	constructor(token?: string) {
		this._token = token;
	}

	updateToken(token: string | undefined) {
		this._token = token;
		if (this._client) {
			this._client = undefined;
		}
	}

	get client(): Octokit {
		if (!this._client) {
			this._client = newClient(this._token);
		}
		return this._client;
	}
}

function newClient(token?: string) {
	return new Octokit({
		auth: token,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com',
		request: {
			// Global rate-limiter to mitigate accidental reactivity bugs that
			// could trigger runaway requests.
			fetch: rateLimit({
				name: 'Octokit',
				limit: 100,
				windowMs: 60 * 1000,
				fn: async (input: RequestInfo | URL, init?: RequestInit) => {
					return await window.fetch(input, init);
				}
			})
		}
	});
}
