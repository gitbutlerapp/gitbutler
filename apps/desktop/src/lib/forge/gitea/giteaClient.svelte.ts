import { InjectionToken } from '@gitbutler/core/context';
import { derived } from 'svelte/store';
import type { GiteaApiPullRequest, GiteaApiUser } from '$lib/forge/gitea/types';
import type { GiteaState } from '$lib/forge/gitea/giteaState.svelte';

export const GITEA_CLIENT = new InjectionToken<GiteaClient>('GiteaClient');

export class GiteaClient {
	private token: string | undefined;
	instanceUrl: string | undefined;
	owner: string | undefined;
	repo: string | undefined;
	private callbacks: (() => void)[] = [];

	constructor(private giteaState: GiteaState) {}

	setRepo(params: { owner?: string; repo?: string }) {
		this.owner = params.owner;
		this.repo = params.repo;
	}

	set() {
		const subscribable = derived(
			[this.giteaState.instanceUrl, this.giteaState.token],
			([instanceUrl, token]) => {
				this.instanceUrl = instanceUrl;
				this.token = token;
				for (const cb of this.callbacks) cb();
			}
		);

		$effect(() => {
			const unsubscribe = subscribable.subscribe(() => {});
			return unsubscribe;
		});
	}

	onReset(fn: () => void) {
		this.callbacks.push(fn);
		return () => (this.callbacks = this.callbacks.filter((cb) => cb !== fn));
	}

	get isAuthenticated(): boolean {
		return !!this.token && !!this.instanceUrl;
	}

	private get baseUrl(): string {
		return `${this.instanceUrl}/api/v1`;
	}

	private get headers(): Record<string, string> {
		return {
			Authorization: `token ${this.token}`,
			'Content-Type': 'application/json'
		};
	}

	private async request<T>(path: string, options?: RequestInit): Promise<T> {
		if (!this.token || !this.instanceUrl) {
			throw new Error('Gitea client not authenticated');
		}

		const response = await fetch(`${this.baseUrl}${path}`, {
			...options,
			headers: { ...this.headers, ...options?.headers }
		});

		if (!response.ok) {
			const errorBody = await response.text().catch(() => '');
			throw new Error(`Gitea API error ${response.status}: ${errorBody}`);
		}

		const text = await response.text();
		if (!text) return undefined as T;
		return JSON.parse(text) as T;
	}

	async getCurrentUser(): Promise<GiteaApiUser> {
		return this.request<GiteaApiUser>('/user');
	}

	async listOpenPulls(owner: string, repo: string): Promise<GiteaApiPullRequest[]> {
		return this.request<GiteaApiPullRequest[]>(
			`/repos/${owner}/${repo}/pulls?state=open&limit=50`
		);
	}

	async listPullsByBranch(
		owner: string,
		repo: string,
		branchName: string
	): Promise<GiteaApiPullRequest[]> {
		const allPrs = await this.listOpenPulls(owner, repo);
		return allPrs.filter((pr) => pr.head.ref === branchName);
	}

	async getPullRequest(
		owner: string,
		repo: string,
		number: number
	): Promise<GiteaApiPullRequest> {
		return this.request<GiteaApiPullRequest>(`/repos/${owner}/${repo}/pulls/${number}`);
	}

	async createPullRequest(
		owner: string,
		repo: string,
		params: { title: string; body: string; head: string; base: string }
	): Promise<GiteaApiPullRequest> {
		return this.request<GiteaApiPullRequest>(`/repos/${owner}/${repo}/pulls`, {
			method: 'POST',
			body: JSON.stringify(params)
		});
	}

	async updatePullRequest(
		owner: string,
		repo: string,
		number: number,
		params: { title?: string; body?: string; state?: string; base?: string }
	): Promise<GiteaApiPullRequest> {
		return this.request<GiteaApiPullRequest>(`/repos/${owner}/${repo}/pulls/${number}`, {
			method: 'PATCH',
			body: JSON.stringify(params)
		});
	}

	async mergePullRequest(
		owner: string,
		repo: string,
		number: number,
		method: 'merge' | 'rebase' | 'squash'
	): Promise<void> {
		await this.request(`/repos/${owner}/${repo}/pulls/${number}/merge`, {
			method: 'POST',
			body: JSON.stringify({
				Do: method,
				delete_branch_after_merge: true
			})
		});
	}
}

export function gitea(extra: unknown): {
	client: GiteaClient;
	owner: string;
	repo: string;
} {
	if (!hasGitea(extra)) throw new Error('No Gitea client!');
	if (!extra.giteaClient.isAuthenticated) throw new Error('Gitea client not authenticated');
	if (!extra.giteaClient.owner || !extra.giteaClient.repo)
		throw new Error('No Gitea repo info set');

	return {
		client: extra.giteaClient,
		owner: extra.giteaClient.owner,
		repo: extra.giteaClient.repo
	};
}

function hasGitea(extra: unknown): extra is {
	giteaClient: GiteaClient;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'giteaClient' in extra &&
		extra.giteaClient instanceof GiteaClient
	);
}
