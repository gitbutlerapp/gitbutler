import { GitHubPrMonitor } from './githubPrMonitor';
import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance, parseGitHubDetailedPullRequest } from './types';
import { showToast } from '$lib/notifications/toasts';
import { sleep } from '$lib/utils/sleep';
import posthog from 'posthog-js';
import { get, writable } from 'svelte/store';
import type { Persisted } from '$lib/persisted/persisted';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHostPrService } from '../interface/gitHostPrService';
import type { DetailedPullRequest, MergeMethod, PullRequest } from '../interface/types';
import type { Octokit } from '@octokit/rest';

const DEFAULT_PULL_REQUEST_TEMPLATE_PATH = '.github/PULL_REQUEST_TEMPLATE.md';

export class GitHubPrService implements GitHostPrService {
	loading = writable(false);

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private baseBranch: string,
		private upstreamName: string,
		private usePullRequestTemplate?: Persisted<boolean>,
		private pullRequestTemplatePath?: Persisted<string>
	) {}

	async createPr(title: string, body: string, draft: boolean): Promise<PullRequest> {
		this.loading.set(true);
		const request = async (pullRequestTemplate: string | undefined = '') => {
			const resp = await this.octokit.rest.pulls.create({
				owner: this.repo.owner,
				repo: this.repo.name,
				head: this.upstreamName,
				base: this.baseBranch,
				title,
				body: body ? body : pullRequestTemplate,
				draft
			});
			return ghResponseToInstance(resp.data);
		};

		let attempts = 0;
		let lastError: any;
		let pr: PullRequest | undefined;
		let pullRequestTemplate: string | undefined;
		const usePrTemplate = this.usePullRequestTemplate ? get(this.usePullRequestTemplate) : null;

		if (!body && usePrTemplate) {
			pullRequestTemplate = await this.fetchPrTemplate();
		}

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				pr = await request(pullRequestTemplate);
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

	async fetchPrTemplate() {
		const path = this.pullRequestTemplatePath
			? get(this.pullRequestTemplatePath)
			: DEFAULT_PULL_REQUEST_TEMPLATE_PATH;

		try {
			const response = await this.octokit.rest.repos.getContent({
				owner: this.repo.owner,
				repo: this.repo.name,
				path
			});
			const b64Content = (response.data as any)?.content;
			if (b64Content) {
				return decodeURIComponent(escape(atob(b64Content)));
			}
		} catch (err) {
			console.error(`Error fetching pull request template at path: ${path}`, err);

			showToast({
				title: 'Failed to fetch pull request template',
				message: `Template not found at path: \`${path}\`.`,
				style: 'neutral'
			});
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

	prMonitor(prNumber: number): GitHubPrMonitor {
		return new GitHubPrMonitor(this, prNumber);
	}
}
