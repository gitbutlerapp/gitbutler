import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance } from './types';
import { showError } from '$lib/notifications/toasts';
import { writable } from 'svelte/store';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { ForgeListingService } from '../interface/forgeListingService';
import type { PullRequest } from '../interface/types';
import type { Octokit } from '@octokit/rest';

export class GitHubListingService implements ForgeListingService {
	readonly prs = writable<PullRequest[]>([], () => {
		this.refresh();
	});

	private error = writable();

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private projectMetrics?: ProjectMetrics
	) {}

	async refresh() {
		if (!navigator.onLine) {
			return;
		}
		try {
			const rsp = await this.octokit.rest.pulls.list({
				headers: DEFAULT_HEADERS,
				owner: this.repo.owner,
				repo: this.repo.name
			});
			const data = rsp.data;
			const prs = data.map((item) => ghResponseToInstance(item));
			this.projectMetrics?.setMetric('pr_count', prs.length);
			this.prs.set(prs);
		} catch (err: unknown) {
			this.error.set(err);
			showError('Failed to fetch PRs', err);
		}
	}
}
