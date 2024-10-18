import { BitBucketBranch } from './bitbucketBranch';
import type { ForgeType } from '$lib/backend/forge';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';
import type { DetailedPullRequest, GitHostArguments } from '../interface/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export const BITBUCKET_DOMAIN = 'bitbucket.org';

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/3252
 */
export class BitBucket implements GitHost {
	readonly type: ForgeType = 'bitbucket';
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;

	constructor({ repo, baseBranch, forkStr }: GitHostArguments) {
		this.baseUrl = `https://${BITBUCKET_DOMAIN}/${repo.owner}/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
	}

	branch(name: string) {
		return new BitBucketBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commits/${id}`;
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
}
