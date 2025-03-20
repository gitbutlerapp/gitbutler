import { rateLimit } from '$lib/utils/ratelimit';
import { Octokit } from '@octokit/rest';

export class GitHubClient {
	private _client: Octokit | undefined;
	private _token: string | undefined;
	private _owner: string | undefined;
	private _repo: string | undefined;

	constructor(args?: {
		// Personal access token for use with Octokit, ignored if `client` provided.
		token?: string;
		// Optional authenticated client.
		client?: Octokit;
	}) {
		this._client = args?.client;
		this._token = args?.token;
	}

	setToken(token: string | undefined) {
		this._token = token;
		if (this._client) {
			this._client = undefined;
		}
	}

	setRepo(info: { owner?: string; repo?: string }) {
		this._owner = info.owner;
		this._repo = info.repo;
	}

	get octokit(): Octokit {
		if (!this._client) {
			this._client = newClient(this._token);
		}
		return this._client;
	}

	get owner(): string | undefined {
		return this._owner;
	}

	get repo(): string | undefined {
		return this._repo;
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
