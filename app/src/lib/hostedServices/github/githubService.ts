import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrMonitor } from './githubPrMonitor';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { HostedGitChecksMonitor } from '../interface/hostedGitChecksMonitor';
import type { HostedGitListingService } from '../interface/hostedGitListingService';
import type { HostedGitPrMonitor } from '../interface/hostedGitPrMonitor';
import type { HostedGitPrService } from '../interface/hostedGitPrService';
import type { HostedGitService } from '../interface/hostedGitService';
import type { DetailedPullRequest } from '../interface/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export class GitHubService implements HostedGitService {
	constructor(
		private projectMetrics: ProjectMetrics,
		private octokit: Octokit,
		private repo: RepoInfo
	) {}

	listService(): HostedGitListingService {
		return new GitHubListingService(this.projectMetrics, this.octokit, this.repo);
	}

	prService(baseBranch: string, upstreamName: string): HostedGitPrService {
		return new GitHubPrService(this.octokit, this.repo, baseBranch, upstreamName);
	}

	prMonitor(prService: GitHubPrService, prNumber: number): HostedGitPrMonitor {
		return new GitHubPrMonitor(prService, prNumber);
	}

	checksMonitor(sourceBranch: string): HostedGitChecksMonitor {
		return new GitHubChecksMonitor(this.octokit, this.repo, sourceBranch);
	}
}
