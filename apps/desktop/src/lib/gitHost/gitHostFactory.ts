import { AZURE_DOMAIN, AzureDevOps } from './azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from './bitbucket/bitbucket';
import { GitHub, GITHUB_DOMAIN } from './github/github';
import { GitLab, GITLAB_DOMAIN } from './gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from './interface/gitHost';
import type { Octokit } from '@octokit/rest';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface GitHostFactory {
	build(repo: RepoInfo, baseBranch: string): GitHost | undefined;
}

export class DefaultGitHostFactory implements GitHostFactory {
	constructor(private octokit: Octokit | undefined) {}

	build(repo: RepoInfo, baseBranch: string, fork?: RepoInfo) {
		const source = repo.source;
		const forkStr = fork ? `${fork.owner}:${fork.name}` : undefined;
		if (source.includes(GITHUB_DOMAIN)) {
			return new GitHub(repo, baseBranch, forkStr, this.octokit, new ProjectMetrics());
		}
		if (source.includes(GITLAB_DOMAIN)) {
			return new GitLab(repo, baseBranch, forkStr);
		}
		if (source.includes(BITBUCKET_DOMAIN)) {
			return new BitBucket(repo, baseBranch, forkStr);
		}
		if (source.includes(AZURE_DOMAIN)) {
			return new AzureDevOps(repo, baseBranch, forkStr);
		}
	}
}
