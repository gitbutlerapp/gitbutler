import { GitHubService } from './github/githubService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHostService } from './interface/gitHostService';
import type { Octokit } from '@octokit/rest';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface GitHostServiceFactory {
	build(repo: RepoInfo): GitHostService | undefined;
}

export class DefaultGitHostServiceFactory implements GitHostServiceFactory {
	constructor(private octokit: Octokit | undefined) {}

	build(repo: RepoInfo): GitHostService | undefined {
		switch (repo.provider) {
			case 'github.com':
				if (!this.octokit) throw new Error('Octokit not available');
				return new GitHubService(new ProjectMetrics(), this.octokit, repo);
		}
		throw new Error('');
	}
}
