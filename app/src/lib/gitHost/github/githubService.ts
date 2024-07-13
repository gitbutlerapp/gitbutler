import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHostChecksMonitor } from '../interface/gitHostChecksMonitor';
import type { GitHostListingService } from '../interface/gitHostListingService';
import type { GitHostPrService } from '../interface/gitHostPrService';
import type { GitHostService } from '../interface/gitHostService';
import type { DetailedPullRequest } from '../interface/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export class GitHubService implements GitHostService {
	constructor(
		private projectMetrics: ProjectMetrics,
		private octokit: Octokit,
		private repo: RepoInfo
	) {}

	listService(): GitHostListingService {
		return new GitHubListingService(this.projectMetrics, this.octokit, this.repo);
	}

	prService(baseBranch: string, upstreamName: string): GitHostPrService {
		return new GitHubPrService(this.octokit, this.repo, baseBranch, upstreamName);
	}

	checksMonitor(sourceBranch: string): GitHostChecksMonitor {
		return new GitHubChecksMonitor(this.octokit, this.repo, sourceBranch);
	}
}
