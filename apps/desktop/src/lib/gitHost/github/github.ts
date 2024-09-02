import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { Persisted } from '$lib/persisted/persisted';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';
import type { GitHostArguments } from '../interface/types';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements GitHost {
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;
	private octokit?: Octokit;
	private projectMetrics?: ProjectMetrics;
	private usePullRequestTemplate?: Persisted<boolean>;
	private pullRequestTemplatePath?: Persisted<string>;

	constructor({
		repo,
		baseBranch,
		forkStr,
		octokit,
		projectMetrics,
		usePullRequestTemplate,
		pullRequestTemplatePath
	}: GitHostArguments & {
		octokit?: Octokit;
		projectMetrics?: ProjectMetrics;
		usePullRequestTemplate?: Persisted<boolean>;
		pullRequestTemplatePath?: Persisted<string>;
	}) {
		this.baseUrl = `https://${GITHUB_DOMAIN}/${repo.owner}/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.octokit = octokit;
		this.projectMetrics = projectMetrics;
		this.usePullRequestTemplate = usePullRequestTemplate;
		this.pullRequestTemplatePath = pullRequestTemplatePath;
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
		return new GitHubPrService(
			this.octokit,
			this.repo,
			baseBranch,
			upstreamName,
			this.usePullRequestTemplate,
			this.pullRequestTemplatePath
		);
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
