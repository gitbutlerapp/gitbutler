import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance } from './types';
import { writable } from 'svelte/store';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { ForgeListingService } from '../interface/forgeListingService';
import type { PullRequest } from '../interface/types';
import type { Octokit } from '@octokit/rest';

export class GitHubListingService implements ForgeListingService {
	readonly prs = writable<PullRequest[]>([], () => {
		this.fetch();
	});

	private error = writable();

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private projectMetrics?: ProjectMetrics
	) {}

	async fetch() {
		const rsp = await this.octokit.rest.pulls.list({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name
		});
		const data = rsp.data;
		const prs = data.map((item) => ghResponseToInstance(item));
		this.prs.set(prs);
		this.projectMetrics?.setMetric('pr_count', prs.length);
		return prs;
	}

	async refresh(): Promise<void> {
		try {
			await this.fetch();
		} catch (e) {
			this.error.set(e);
			console.error(e);
		}
	}
}
