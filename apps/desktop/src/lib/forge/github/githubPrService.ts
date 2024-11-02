import { GitHubPrMonitor } from './githubPrMonitor';
import { DEFAULT_HEADERS } from './headers';
import {
	ghResponseToInstance,
	parseGitHubDetailedPullRequest,
	type GitHubPullRequestId
} from './types';
import {
	ForgeName,
	type CreatePullRequestArgs,
	type DetailedPullRequest,
	type PullRequestId,
	type MergeMethod,
	type PullRequest
} from '$lib/forge/interface/types';
import { sleep } from '$lib/utils/sleep';
import posthog from 'posthog-js';
import { writable } from 'svelte/store';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Octokit } from '@octokit/rest';

const INVALID_PR_TYPE = new Error(`Forge name mismatch, expected 'GitHub'`);

export class GitHubPrService implements ForgePrService {
	loading = writable(false);

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo
	) {}

	async createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest> {
		this.loading.set(true);
		const request = async () => {
			const resp = await this.octokit.rest.pulls.create({
				owner: this.repo.owner,
				repo: this.repo.name,
				head: upstreamName,
				base: baseBranchName,
				title,
				body,
				draft
			});

			return ghResponseToInstance(resp.data);
		};

		let attempts = 0;
		let lastError: any;
		let pr: PullRequest | undefined;

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				pr = await request();
				posthog.capture('PR Successful');
				return pr;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		throw lastError;
	}

	async get(id: PullRequestId): Promise<DetailedPullRequest> {
		if (!isGitHubPr(id)) throw INVALID_PR_TYPE;
		const resp = await this.octokit.pulls.get({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			pull_number: id.subject.prNumber
		});
		return parseGitHubDetailedPullRequest(resp.data);
	}

	async merge(method: MergeMethod, id: PullRequestId) {
		if (!isGitHubPr(id)) throw INVALID_PR_TYPE;
		await this.octokit.pulls.merge({
			owner: this.repo.owner,
			repo: this.repo.name,
			pull_number: id.subject.prNumber,
			merge_method: method
		});
	}

	async reopen(id: PullRequestId) {
		if (!isGitHubPr(id)) throw INVALID_PR_TYPE;
		await this.octokit.pulls.update({
			owner: this.repo.owner,
			repo: this.repo.name,
			pull_number: id.subject.prNumber,
			state: 'open'
		});
	}

	prMonitor(id: PullRequestId): GitHubPrMonitor {
		if (!isGitHubPr(id)) throw INVALID_PR_TYPE;
		return new GitHubPrMonitor(this, id);
	}
}

function isGitHubPr(
	id: PullRequestId
): id is { type: ForgeName.GitHub; subject: GitHubPullRequestId } {
	return id.type === ForgeName.GitHub;
}
