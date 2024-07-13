import { GitHubService } from './github/githubService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { HostedGitService } from './interface/hostedGitService';
import type { Octokit } from '@octokit/rest';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface HostedGitServiceFactory {
	build(repo: RepoInfo): HostedGitService | undefined;
}

export class DefaultHostedGitServiceFactory implements HostedGitServiceFactory {
	constructor(private octokit: Octokit | undefined) {}

	build(repo: RepoInfo): HostedGitService | undefined {
		switch (repo.provider) {
			case 'github.com':
				if (!this.octokit) throw new Error('Octokit not available');
				return new GitHubService(new ProjectMetrics(), this.octokit, repo);
		}
		throw new Error('');
	}
}
