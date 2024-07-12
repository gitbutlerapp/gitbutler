import { mapErrorToToast } from './errorMap';
import { GitHubPrMonitor } from './githubPrMonitor';
import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance, parseGitHubDetailedPullRequest } from './types';
import { showToast } from '$lib/notifications/toasts';
import posthog from 'posthog-js';
import { writable } from 'svelte/store';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { HostedGitPrMonitor } from '../interface/hostedGitPrMonitor';
import type { HostedGitPrService } from '../interface/hostedGitPrService';
import type { DetailedPullRequest, MergeMethod, PullRequest } from '../interface/types';
import type { Octokit } from '@octokit/rest';

export class GitHubPrService implements HostedGitPrService {
	loading = writable(false);

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private baseBranch: string,
		private upstreamName: string
	) {}

	async createPr(title: string, body: string, draft: boolean): Promise<PullRequest | undefined> {
		this.loading.set(true);
		try {
			const resp = await this.octokit.rest.pulls.create({
				owner: this.repo.owner,
				repo: this.repo.name,
				head: this.upstreamName,
				base: this.baseBranch,
				title,
				body,
				draft
			});
			posthog.capture('PR Successful');
			return ghResponseToInstance(resp.data);
		} catch (err: any) {
			const toast = mapErrorToToast(err);
			if (toast) {
				showToast(toast);
			} else {
				// Rethrow so that error is retried
				throw err;
			}
		} finally {
			this.loading.set(false);
		}
	}

	async get(prNumber: number): Promise<DetailedPullRequest> {
		const resp = await this.octokit.pulls.get({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			pull_number: prNumber
		});
		return parseGitHubDetailedPullRequest(resp.data);
	}

	async merge(method: MergeMethod, prNumber: number) {
		await this.octokit.pulls.merge({
			owner: this.repo.owner,
			repo: this.repo.name,
			pull_number: prNumber,
			merge_method: method
		});
	}

	prMonitor(prNumber: number): HostedGitPrMonitor {
		return new GitHubPrMonitor(this, prNumber);
	}
}
