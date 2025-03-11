import { DEFAULT_HEADERS } from './headers';
import { ghResponseToInstance } from './types';
import { showError } from '$lib/notifications/toasts';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
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
	private disabled = false;

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private projectMetrics?: ProjectMetrics
	) {}

	async refresh() {
		if (!navigator.onLine || this.disabled) {
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
			// Suppress error if there is a repo restriction on using personal access tokens. This is a
			// a bit of a hack, and should be reworked into some persistent state that can disable the
			// integration for a specific repo and give the user appropriate feedback.
			if (
				isErrorlike(err) &&
				err.message.includes('you appear to have the correct authorization credentials')
			) {
				this.disabled = true;
				return;
			}
			this.error.set(err);
			showError('Failed to fetch PRs', err);
		}
	}
}
