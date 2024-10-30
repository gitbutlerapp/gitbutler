import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { GitHubIssueService } from '$lib/forge/github/issueService';
import { Octokit } from '@octokit/rest';
import type { ForgeType } from '$lib/backend/forge';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Forge } from '../interface/forge';
import type { ForgeArguments } from '../interface/types';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements Forge {
	readonly type: ForgeType = 'github';
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;
	private octokit?: Octokit;
	private projectMetrics?: ProjectMetrics;

	constructor({
		repo,
		baseBranch,
		forkStr,
		octokit,
		projectMetrics
	}: ForgeArguments & {
		octokit?: Octokit;
		projectMetrics?: ProjectMetrics;
	}) {
		this.baseUrl = `https://${GITHUB_DOMAIN}/${repo.owner}/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.octokit = octokit;
		this.projectMetrics = projectMetrics;
	}

	listService() {
		if (!this.octokit) {
			return;
		}
		return new GitHubListingService(this.octokit, this.repo, this.projectMetrics);
	}

	prService() {
		if (!this.octokit) {
			return;
		}
		return new GitHubPrService(this.octokit, this.repo);
	}

	issueService() {
		if (!this.octokit) {
			return;
		}
		return new GitHubIssueService(this.octokit, this.repo);
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
		return new GitHubBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}
}
