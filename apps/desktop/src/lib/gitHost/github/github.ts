import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { Settings } from '$lib/settings/userSettings';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';
import type { GitHostArguments } from '../interface/types';
import type { Readable } from 'svelte/store';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements GitHost {
	baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;
	private octokit?: Octokit;
	private projectMetrics?: ProjectMetrics;
	private userSettings?: Readable<Settings>;

	constructor({
		repo,
		baseBranch,
		forkStr,
		octokit,
		projectMetrics,
		userSettings
	}: GitHostArguments & {
		octokit?: Octokit;
		projectMetrics?: ProjectMetrics;
		userSettings?: Readable<Settings>;
	}) {
		this.baseUrl = `https://${GITHUB_DOMAIN}/${repo.owner}/${repo.name}`;

		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.octokit = octokit;
		this.projectMetrics = projectMetrics;
		this.userSettings = userSettings;
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
			this.userSettings
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
