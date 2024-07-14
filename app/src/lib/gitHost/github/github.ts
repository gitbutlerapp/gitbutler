import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHostService';
import type { DetailedPullRequest } from '../interface/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export class GitHub implements GitHost {
	constructor(
		private projectMetrics: ProjectMetrics,
		private octokit: Octokit,
		private repo: RepoInfo
	) {}

	listService() {
		return new GitHubListingService(this.projectMetrics, this.octokit, this.repo);
	}

	prService(baseBranch: string, upstreamName: string) {
		return new GitHubPrService(this.octokit, this.repo, baseBranch, upstreamName);
	}

	checksMonitor(sourceBranch: string) {
		return new GitHubChecksMonitor(this.octokit, this.repo, sourceBranch);
	}
}
