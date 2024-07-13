import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance } from './types';
import { writable } from 'svelte/store';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHostListingService } from '../interface/gitHostListingService';
import type { PullRequest } from '../interface/types';
import type { Octokit } from '@octokit/rest';

export class GitHubListingService implements GitHostListingService {
	readonly prs = writable<PullRequest[]>([], () => {
		this.fetchPrs();
	});

	private error = writable();

	constructor(
		private projectMetrics: ProjectMetrics,
		private octokit: Octokit,
		private repo: RepoInfo
	) {}

	private async fetchPrs() {
		try {
			const rsp = await this.octokit.rest.pulls.list({
				headers: DEFAULT_HEADERS,
				owner: this.repo.owner,
				repo: this.repo.name
			});
			const data = rsp.data;
			this.projectMetrics.setMetric('pr_count', data.length);
			this.prs.set(data.map(ghResponseToInstance));
		} catch (e) {
			this.error.set(e);
			console.error(e);
		}
	}

	async reload(): Promise<void> {
		return await this.fetchPrs();
	}
}
