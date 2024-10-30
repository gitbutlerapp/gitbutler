import { AZURE_DOMAIN, AzureDevOps } from './azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from './bitbucket/bitbucket';
import { GitHub, GITHUB_DOMAIN } from './github/github';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from './gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Forge } from './interface/forge';
import type { Octokit } from '@octokit/rest';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface ForgeFactory {
	build(repo: RepoInfo, baseBranch: string): Forge | undefined;
}

export class DefaultForgeFactory implements ForgeFactory {
	constructor(private octokit: Octokit | undefined) {}

	build(repo: RepoInfo, baseBranch: string, fork?: RepoInfo) {
		const domain = repo.domain;
		const forkStr = fork ? `${fork.owner}:${fork.name}` : undefined;

		if (domain.includes(GITHUB_DOMAIN)) {
			return new GitHub({
				repo,
				baseBranch,
				forkStr,
				octokit: this.octokit,
				projectMetrics: new ProjectMetrics()
			});
		}
		if (domain === GITLAB_DOMAIN || domain.startsWith(GITLAB_SUB_DOMAIN + '.')) {
			return new GitLab({ repo, baseBranch, forkStr });
		}
		if (domain.includes(BITBUCKET_DOMAIN)) {
			return new BitBucket({ repo, baseBranch, forkStr });
		}
		if (domain.includes(AZURE_DOMAIN)) {
			return new AzureDevOps({ repo, baseBranch, forkStr });
		}
	}
}
