import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements GitHost {
	baseUrl: string;

	constructor(
		private repo: RepoInfo,
		private baseBranch?: string,
		private fork?: string,
		private octokit?: Octokit,
		private projectMetrics?: ProjectMetrics
	) {
		this.baseUrl = `https://${GITHUB_DOMAIN}/${repo.owner}/${repo.name}`;
	}

	listService() {
		if (!this.octokit) {
			return;
		}
		return new GitHubListingService(this.octokit, this.repo, this.projectMetrics);
	}

	prService(baseBranch: string, upstreamName: string) {
		if (!this.octokit) {
			return;
		}
		return new GitHubPrService(this.octokit, this.repo, baseBranch, upstreamName);
	}

	checksMonitor(sourceBranch: string) {
		if (!this.octokit) {
			return;
		}
		return new GitHubChecksMonitor(this.octokit, this.repo, sourceBranch);
	}

	branch(name: string) {
		if (!this.baseBranch) {
			return;
		}
		return new GitHubBranch(name, this.baseBranch, this.baseUrl, this.fork);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}
}
