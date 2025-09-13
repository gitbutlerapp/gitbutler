import { rateLimit } from '$lib/utils/ratelimit';
import { InjectionToken } from '@gitbutler/core/context';
import { Octokit } from '@octokit/rest';
import type { ApiClient } from '$lib/forge/interface/apiClient';

export const GITHUB_CLIENT = new InjectionToken<GitHubClient>('GitHubClient');

export class GitHubClient implements ApiClient {
	private _client: Octokit | undefined;
	private _token: string | undefined;
	private _owner: string | undefined;
	private _repo: string | undefined;
	private _domain: string | undefined;
	private subscriptions: (() => void)[] = [];

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
		this.subscriptions.every((cb) => cb());
	}

	onReset(fn: () => void) {
		this.subscriptions.push(fn);
		return () => (this.subscriptions = this.subscriptions.filter((cb) => cb !== fn));
	}

	setRepo(info: { owner?: string; repo?: string; domain?: string }) {
		this._owner = info.owner;
		this._repo = info.repo;
		this._domain = info.domain;
		// Reset client when repo info changes so baseUrl gets updated
		if (this._client) {
			this._client = undefined;
		}
	}

	get octokit(): Octokit {
		if (!this._client) {
			this._client = newClient(this._token, this._domain);
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

function newClient(token?: string, domain?: string) {
	// Construct the appropriate baseUrl based on the domain
	// For github.com or undefined domain, use the standard GitHub API
	// For GitHub Enterprise, use https://<domain>/api as specified in the GitHub Enterprise documentation
	let baseUrl = 'https://api.github.com';
	if (domain && domain !== 'github.com') {
		baseUrl = `https://${domain}/api`;
	}

	return new Octokit({
		auth: token,
		userAgent: 'GitButler Client',
		baseUrl,
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
