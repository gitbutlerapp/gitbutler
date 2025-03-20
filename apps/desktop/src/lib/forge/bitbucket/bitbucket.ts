import { BitBucketBranch } from '$lib/forge/bitbucket/bitbucketBranch';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { DetailedPullRequest, ForgeArguments } from '$lib/forge/interface/types';

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
export class BitBucket implements Forge {
	readonly name: ForgeName = 'bitbucket';
	private baseUrl: string;
	private baseBranch: string;
	private forkStr?: string;

	constructor({ repo, baseBranch, forkStr }: ForgeArguments) {
		this.baseUrl = `https://${BITBUCKET_DOMAIN}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
	}

	branch(name: string) {
		return new BitBucketBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commits/${id}`;
	}

	get listService() {
		return undefined;
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		return undefined;
	}

	get repoService() {
		return undefined;
	}

	get checks() {
		return undefined;
	}
}
