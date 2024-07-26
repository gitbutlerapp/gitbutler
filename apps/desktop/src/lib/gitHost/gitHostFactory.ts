import { GitHub } from './github/github';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from './interface/gitHost';
import type { Octokit } from '@octokit/rest';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface GitHostFactory {
	build(repo: RepoInfo): GitHost | undefined;
}

export class DefaultGitHostFactory implements GitHostFactory {
	constructor(private octokit: Octokit | undefined) {}

	build(repo: RepoInfo) {
		switch (repo.provider) {
			case 'github.com':
				if (!this.octokit) throw new Error('Octokit not available');
				return new GitHub(this.octokit, repo, new ProjectMetrics());
		}
	}
}
