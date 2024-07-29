import { BitBucketBranch } from './bitbucketBranch';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';
import type { DetailedPullRequest } from '../interface/types';

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
	webUrl: string;

	constructor(
		repo: RepoInfo,
		private baseBranch: string,
		private fork?: string
	) {
		this.webUrl = `https://${BITBUCKET_DOMAIN}/${repo.owner}/${repo.name}`;
	}

	branch(name: string) {
		return new BitBucketBranch(name, this.baseBranch, this.webUrl, this.fork);
	}

	commitUrl(id: string): string {
		return `${this.webUrl}/commits/${id}`;
	}

	listService() {
		return undefined;
	}

	prService(_baseBranch: string, _upstreamName: string) {
		return undefined;
	}

	checksMonitor(_sourceBranch: string) {
		return undefined;
	}
}
