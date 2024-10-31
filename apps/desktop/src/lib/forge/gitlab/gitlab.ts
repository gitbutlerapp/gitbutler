import { GitLabBranch } from './gitlabBranch';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Forge, ForgeName } from '../interface/forge';
import type { DetailedPullRequest, ForgeArguments } from '../interface/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export const GITLAB_DOMAIN = 'gitlab.com';
export const GITLAB_SUB_DOMAIN = 'gitlab'; // For self hosted instance of Gitlab

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2511
 */
export class GitLab implements Forge {
	readonly name: ForgeName = 'gitlab';
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;

	constructor({ repo, baseBranch, forkStr }: ForgeArguments) {
		this.baseUrl = `https://${repo.domain}/${repo.owner}/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
	}

	branch(name: string) {
		return new GitLabBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
	}

	listService() {
		return undefined;
	}

	issueService() {
		return undefined;
	}

	prService() {
		return undefined;
	}

	checksMonitor(_sourceBranch: string) {
		return undefined;
	}

	async pullRequestTemplateContent(_path?: string) {
		return undefined;
	}
}
